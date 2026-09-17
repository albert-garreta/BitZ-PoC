# GKR and compact-coefficient optimization results

Status: the two retained implementations are committed and focused final
validation is complete. The four areas were evaluated, but two were rejected
under the prover-first regression policy. No speedups from separate experiments
are added together.

## Rebase onto matrix-binding

The full 12-commit optimization series has been rebased onto `matrix-binding`
(`f91b66f1`). The original tip is retained as
`codex/gkr-optimizations-before-matrix-binding-20260916` (`1090d2a9`).
Commit IDs and performance measurements below describe the pre-rebase campaign;
those timings have not been remeasured on the combined branch.

Compact output now uses the shared reverse traversal in
`crates/circuit/src/linear_map/evaluate.rs` and the circuit adapter's
`adjoint_map_structured`. `CompositeCoefficients` in `src/sumcheck/bridge/`
owns the compact tail, and the borrowed adapter in
`src/sumcheck/inner/packed/composite/compact.rs` feeds the unified inner-sumcheck
entry point. The matrix-binding forward verifier and dense fallback stay in place.
Both branches' benchmark reporting is preserved.

Rebase validation: 477 library tests, 468 nonparallel library tests, and 70
circuit tests passed. The transcript-pin, virtual-opening, and forest-grinding
suites passed (26 tests), as did the separate ECDSA proof-byte regression across
both outer modes and security targets 100/128. Existing proof digests and
transcript continuation pins were retained without changes. Release checks of
all enabled benchmarks and examples passed with `ecdsa,bench-internals,span-metrics`.

## Delivered changes

| Package | Decision | Result |
| --- | --- | --- |
| #18: analytic constants | Retain | Padding contributes algebraically; stored levels, regenerated layers, and prepared grids contain only real groups. |
| #23: bit-driven kernels | Reject | Broad and narrowed defaults caused confirmed regressions. No new lookup kernels or dispatch changes remain in production. |
| #17, #20–21: workspace/bookkeeping | Reject | The combined package slowed serial warm proving and first proofs. Its workspace, prefix factoring, and flat power-table changes were removed together. |
| #9–10: compact coefficients | Retain | One validated Montgomery tail owns geometric runs and literals. Reverse-tape emission and bounded inner folding avoid dense-tail copies and staging arrays. |

Public proving APIs, transcript operations, proof format, protocol parameters,
and security settings are unchanged. The verifier's forward-tape evaluator is
unchanged. The six refactoring commits between baseline `8598a1a6` and parent
`c2a6281e` remain intact.

For padded GKR trees, `C = sum_t eq(z_c,t)`. Round `j` contributes
`C * A_j * eq_1(X,q_j)`. The initial claim includes `C`; the existing running
claim carries subsequent constant coefficients. No inverse is used, including
when an equality prefix is zero. Padded final `(1,1)` values retain their tree
ordering. The tiling threshold is unchanged: 512-entry materialization halves
in the main fixture remain below the 4,096-entry default.

The coefficient owner validates segment order, bounds, literal coverage, and
canonical Montgomery words for one field context. Gaps mean zero; public-input
corrections split runs and insert literals. The borrowed inner adapter processes
bounded intersections and writes the final folded Montgomery table directly.
It never reinterprets `[u64; 2]` allocations as `u128` allocations. Independent
expanded-matrix gathers remain the correctness oracle.

## Measurement method and scope

AMD Ryzen 9 9950X3D, Linux; CPU 0 for one thread and CPUs 0–9 for ten threads.
Native benchmarks use `-C target-cpu=native -C force-frame-pointers=yes`, fat
LTO, and one codegen unit. Frozen binaries are hashed. Compilation, tests,
allocation instrumentation, and Git maintenance do not overlap timed runs.
First-proof and warm measurements exclude setup. Target 100 uses the existing
`custom:1:4` tuning profile; target 128 uses the validated target-specific
default in both compared executables. Paired intervals bootstrap
process-level ratios, rather than treating warm repetitions as independent.
Displayed times are marginal sample medians; ratio intervals use paired process
medians, so the ratio of displayed medians is a different estimator.

At the user's request to finish promptly, final validation uses six paired
blocks with three warm repetitions for the focused SHA/P256 comparisons and a
short two-block screen across multiplication, SHA chains, and MultiSwap. This
screen checks proof compatibility and large regressions; it does **not** meet
the original exhaustive 6/12/24-block acceptance campaign for every workload.
Unresolved results are not labeled demonstrated improvements.

Corresponding proof-message/PCS-byte fingerprints and transcript continuation
must match. SHA-chain trials intentionally use different deterministic seeds;
comparison is between corresponding trials across executables. Allocation runs
are separate from latency runs. Peak live Rust heap, cumulative allocated bytes,
allocation calls, and process RSS are different metrics.

## Earlier incremental measurements

These are tuning results, not final combined speedup claims. Times are ms.

| Comparison: 1,024 compressions + P-256 | Threads | GKR before → after | Witness-to-proof before → after |
| --- | ---: | ---: | ---: |
| Analytic constants vs immediate parent | 1 | 241.55 → 227.86 | 463.93 → 451.78 |
| Analytic constants vs immediate parent | 10 | 37.77 → 35.05 | 87.98 → 85.88 |
| Compact coefficients vs the old workspace prototype | 1 | 218.95 → 218.77 | 432.95 → 422.22 |
| Compact coefficients vs the old workspace prototype | 10 | 33.49 → 34.61 | 81.62 → 69.50 |

The last row improves total proving while slowing its GKR phase; it is not an
incremental GKR win. The final retained implementation excludes that workspace
and kernel prototype and is measured separately below.

## Rejected experiments

The unrestricted kernel screen covered 32 workload/thread configurations.
First-proof ratio intervals were `[1.0101, 1.0438]` for native u32 at `2^15`, ten
threads, and `[1.0198, 1.0647]` for 128-compression SHA chains at ten threads.
A narrowed dispatch still slowed warm native-u32 proving: **15.237 → 15.412 ms**,
interval `[1.0086, 1.0305]`. The candidate is rejected even though other shapes
improved. Unroll factors 1/2/4 and shared-table weighting remain experiments.

The final workspace revision (`step3-slots-focused`) also failed:

| Main SHA/P256 fixture | Before | After | Paired ratio interval |
| --- | ---: | ---: | --- |
| One thread, warm witness-to-proof | 434.97 ms | 438.79 ms | [1.0055, 1.0198] |
| One thread, first witness-to-proof | 440.56 ms | 447.11 ms | [1.0038, 1.0279] |
| Ten threads, first witness-to-proof | 100.09 ms | 101.21 ms | [1.0046, 1.0450] |

The first dense-cache prototype and its no-cache/serial-allocation revision
also regressed. Removing output-slot traversal and restoring worker allocation
did not resolve the problem. Removing the entire package does not establish
which individual change caused the slowdown. Further isolation is deferred.

## Correctness and artifact audit

Before removing the rejected packages, the implementation passed 482 root
library tests, 15 isolated override runs, 42 integration tests, the explicitly
enabled ECDSA regression, and 473 nonparallel library tests. Library runs also
passed their isolated child-process checks. Circuit tests passed 48 cases,
vendor field tests 107, and vendor Flock tests 293. Ignored tests are not counted
as passes. The retained source is checked again during finalization.

Differential tests cover zero/Boolean challenges, zero equality prefixes,
all-zero/padded witnesses, tiny trees, multiple primes, both outer modes,
public-input exceptions, prefix widths 0–4, misaligned starts, and partial
blocks. No full-tail descriptor expansion is used as its own oracle.

Raw artifacts are in `PerfRuns/analytic-20260916/`. Invalid `step2-v2` and
`step4-v2` builds reused stale local Cargo packages and are excluded. One
interrupted campaign overlapped an orphan and is excluded. Subsequent snapshot
builds explicitly clean local packages and require compilation in their logs.
A failed focused launch omitted the native trace-processor path; it produced no
accepted timing samples and was rerun with the correct environment. An initial target-128 launch inherited the target-100 profile and was
rejected by the baseline security-bound check; the harness now uses the
validated target-specific default for 128, without lowering the target. The Flock
suite passed offline after a registry lookup failed; its tracked lockfile was
restored. Rejected implementations are saved as local experiment artifacts,
not included in the production source.

## Final retained implementation

Final latency, verifier, and allocation results follow. The full held-out 6/12/24-block matrix remains outside this
shortened finalization; no universal no-regression claim is made.

### Allocation and peak-memory capture

One fresh instrumented process per variant, main 1,024-compression SHA/P256
fixture, ten threads. Proof digests matched. These instrumented timings are not
used as latency evidence. Compilation ran concurrently with this allocation-only
capture; process-local heap counters are independent, and RSS is a single
observation rather than a statistically established improvement.

| Metric | Baseline | Retained implementation |
| --- | ---: | ---: |
| Proving cumulative allocated bytes | 782.78 MiB | 470.13 MiB |
| Proving allocation calls | 1,419,897.00  | 234,891.00  |
| Proving peak live heap | 220.30 MiB | 220.30 MiB |
| Coefficient-phase peak live heap | 186.39 MiB | 131.02 MiB |
| Inner-sumcheck peak live heap | 193.00 MiB | 150.24 MiB |
| Whole-process peak live heap, including setup | 799.25 MiB | 799.25 MiB |
| Process maximum RSS, including setup | 673.19 MiB | 673.64 MiB |

Proving allocation volume falls by 39.9%, and allocation calls by 83.5%.
The proving peak is unchanged because `mf:build_levels` still dominates.
Setup also remains the overall process high-water mark. Phase peaks include
all live Rust heap at that time, including retained prepared data. Nested phase
counters overlap and must not be summed. There is no retained workspace cache
in the delivered implementation.

### Final latency versus `8598a1a6`

Main 1,024-compression SHA/P256 fixture, split outer mode, target 100, fresh
seed 31. Six paired blocks and three warm proofs per process. Times are ms;
intervals are candidate/baseline ratios.

| Threads | Metric | Before | After | Paired 95% interval |
| ---: | --- | ---: | ---: | --- |
| 1 | Warm witness-to-proof | 463.449 | 430.405 | [0.9242, 0.9474] |
| 1 | First witness-to-proof | 473.572 | 435.705 | [0.9114, 0.9371] |
| 1 | Warm GKR | 241.459 | 228.526 | [0.9372, 0.9799] |
| 1 | Warm verification | 21.894 | 21.958 | [1.0002, 1.0033] |
| 10 | Warm witness-to-proof | 88.168 | 70.200 | [0.7698, 0.7942] |
| 10 | First witness-to-proof | 103.666 | 85.228 | [0.7920, 0.8466] |
| 10 | Warm GKR | 38.179 | 35.280 | [0.9002, 0.9425] |
| 10 | Warm verification | 7.571 | 7.520 | [0.9606, 1.0062] |

### Compact coefficients versus analytic constants

Ten threads, 1,024 compressions, fresh seed 17, six paired blocks. These are
incremental total-prover comparisons, not additional speedups to add to the
baseline comparison above.

| Outer mode | Target | Warm before → after (ms) | Warm ratio interval | First-proof before → after (ms) | First-proof ratio interval |
| --- | ---: | --- | --- | --- | --- |
| f2z-split | 100 | 85.531 → 69.846 | [0.7953, 0.8247] | 102.211 → 86.786 | [0.8269, 0.8614] |
| f2z-split | 128 | 127.963 → 112.263 | [0.8498, 0.8816] | 143.218 → 129.849 | [0.8826, 0.9470] |
| f2z-all | 128 | 117.284 → 101.159 | [0.8591, 0.9037] | 131.232 → 115.568 | [0.8486, 0.9392] |
| f2z-all | 100 | 89.382 → 73.492 | [0.8101, 0.8326] | 104.088 → 86.300 | [0.8264, 0.8559] |

### Final correctness and short workload screen

The retained source passes **479 root library tests**, **470 nonparallel library
tests**, the isolated library child checks, **26 integration tests** covering
transcript pins, virtual openings and grinding, and the explicitly enabled
ECDSA regression. Earlier unchanged circuit/vendor suites also passed. The final
benchmark binaries' source hashes match the delivered implementation.

All 32 workload/thread cells in the short screen verified and matched the
baseline's corresponding proof and transcript fingerprints. Coverage includes
native u32/u64/u128 (`2^15` and `2^19`), full u32 W1/W8, SHA chains (128/1,024/4,096
compressions), and MultiSwap batches 1/4/8, each at one and ten threads, using
fresh seed 17. Two paired blocks with two warm repetitions are insufficient to
establish no slowdown in every cell. The detailed ratios, including apparent
slowdowns and uncertain cases, are retained without filtering in the
[machine-readable results](../benchmarks/results/gkr-compact-coefficients-20260916/summary.json).

The original full held-out 6/12/24-block matrix remains deferred following the
request to finish promptly. Only two of the four optimization packages are
shipped; rejected implementations are not hidden behind an enabled default.

### Focused follow-up of screen flags

The short screen's small-SHA cold-prover ratio was 1.059. Six additional paired
blocks did not establish a repeatable slowdown: first-proof time was
**36.978 → 37.013 ms**, ratio interval **[0.9360, 1.1064]**. Warm total time was
35.534 → 36.115 ms, interval [0.9582, 1.0337]. These results remain inconclusive;
they are not reported as a no-regression pass or an improvement. The original
12/24-block escalation is deferred under the shortened finalization scope.

The main serial verifier measurement is 0.3% slower (21.894 → 21.958 ms);
its interval is slightly above one. The ten-thread verifier interval spans
one. This measured serial tradeoff is reported under the prover-first policy.
Absolute results for the broad screen are also available as a
[CSV table](../benchmarks/results/gkr-compact-coefficients-20260916/workload-screen.csv).

The small full-u32 W1 screen's warm point estimate was 4.7% slower. Six additional
paired blocks instead measured **15.743 → 15.727 ms**, interval
**[0.9890, 1.0105]**; first-proof time was 18.433 → 18.284 ms, interval
[0.9418, 1.0149]. The corresponding W8 follow-up remained inconclusive too:
warm 14.769 → 14.602 ms, interval [0.9805, 1.0192], and first-proof
17.006 → 17.538 ms, interval [0.9410, 1.0539]. These checks did not establish
a repeatable whole-prover slowdown, but they also do not establish no regression.
Both follow-up datasets are included in the machine-readable results.

Delivered commits: `3550015c` (analytic constants), `7eb84162` (compact
coefficients), and `b82ac734` (benchmark validation/cleanup), followed by this
report commit. The four requested areas therefore produce **two retained
optimization packages**, with the other two rejected under the regression
policy rather than shipped for the sake of completing four code packages.

An additional unresolved screen flag is MultiSwap batch 1 at ten threads:
first-proof time was **124.661 → 132.321 ms** (+6.1%) in the two-block screen.
Both paired observations were slower. This case was not expanded before the
requested prompt finalization, so the retained series has **not** earned a
full no-regression signoff. That flag, the inconclusive follow-ups above, and
other small point-estimate slowdowns require the deferred longer campaign.
