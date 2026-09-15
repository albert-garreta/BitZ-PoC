# Regression repair campaign

The original measurements remain in `results/optimization-arm-01` and
`results/optimization-arm-10cores-01`. This revision changes only the isolated
experiment. ARM and portable implementations are in scope; production and x86
code are unchanged.

## Changes under measurement

- Compile integer operation/signedness into batch kernels. Use native one- and
  two-word arithmetic, or the existing crypto-bigint carry primitive where it
  generates better code. Preserve the full `Option<Uint>` checked result.
- Accumulate exact unsigned products in wide columns, propagate carries once,
  and validate the public batch-capacity bound once. The output still has
  `2L+1` words. BigUint validates the result, including short batches and tails.
- Write packing output through spare capacity, initialize every value/padding
  slot once, and use coarse parallel work for large buffers. Retain the returned
  packed buffer through OOD evaluation.
- Compare equality-table and OOD block schedules while retaining the actual
  extracted production implementation as the baseline.
- Reuse the tested ARM half-width multiplier inside NTT butterflies. Compare
  depth-first and tiled schedules with equal reset copies and complete outputs.
- Keep explicitly public-input polynomial arithmetic separate from the
  fixed-schedule private-input kernel. The revised public path checks each
  row's actual contents, skips zero rows, compacts sparse operands, and fuses
  products directly into the accumulator.
  Correctness and performance cases include all-zero inputs, alternating rows,
  and prefixes that misrepresent later terms. The earlier prefix-sampling
  candidate remains a diagnostic. Sampling is never added to the fixed-schedule
  entry point.

## Allocation audit correction

A temporary allocation backtrace identified the intermittent 1,520-byte event
as `crossbeam_deque::Injector::push` allocating a Rayon job-queue block. The
captured event occurred in the **production NTT baseline**, too. The trace is
preserved in `results/repair-dev-10cores-03/run-0.log`; that instrumented build
is development evidence only.

The prior three-pass maximum could happen to cross a queue-block boundary in
one variant and miss it in another. The audit now uses **64 passes**, covering
at least two periods of the 31-job injector block. It still reports the maximum
allocation count/bytes for **one call**, includes scheduler allocations, and
retains the original comparison limits. The tiled candidate also enters the
worker pool once for a complete transform, reducing external job submissions.
Timing measurements remain separate from allocation auditing.

## Confirmation policy

Development probes are labeled as such and preserve their raw pairs. They are
used to choose implementations before fresh confirmation, never presented as
passing the final gate. Confirmation keeps five fresh processes, 32 paired
samples, the 1% latency limit, P95/per-process checks, and at most one prescribed
64-sample retry for an inconclusive workload. Rejected variants remain visible.

`results/repair-arm-01` was stopped before completion to add the missing mixed
and misleading-prefix performance cases. Its metadata says `aborted`; its
partial timings were not used to select candidates or claim confirmation.

Where a specialization has no dependable advantage, retaining the actual
baseline is an explicit implementation choice, not a speedup. Private-input
timing guarantees cannot be exchanged for the public-input sparse path.

## Final confirmation and replay

The final one-thread run (`repair-arm-02`, revision 2) passes all 271 selected
checks; 105 explicitly retain the baseline. The final ten-thread run
(`repair-arm-10cores-03`, revision 3) passes all 19 selected checks; seven retain
the baseline. Neither gate has a remaining regression or inconclusive selection.
All required variants pass correctness, including rejected diagnostic kernels.

In `repair-arm-10cores-02`, the log15/32-lane and log16/32-lane NTT candidates
remained inconclusive on tail latency after the prescribed retry. Revision 3
retains production for those shapes and was frozen before a fresh confirmation.
The earlier attempt remains available; its better medians are not claimed as
accepted improvements. Rejected diagnostic variants can still regress.

Both final runs use the exact executable in
`results/repair-arm-02/source-and-binary.tar.gz`. Its `archive.json` records the
archive and binary SHA-256 hashes. The archive includes the measured source tree
and Cargo lockfile without build caches. This is an ARM macOS executable, and
its `target-cpu=native` results do not establish performance on another host.

From the repository root, verify the recorded gates without rebuilding:

```sh
python3 experiments/field-regressions/gate.py experiments/field-regressions/results/repair-arm-02
python3 experiments/field-regressions/gate.py experiments/field-regressions/results/repair-arm-10cores-03
python3 -m unittest discover -s experiments/field-regressions -p 'test_*.py'
```

To rebuild and confirm on an ARM host with dependencies cached, use a new output
directory, then reuse that executable for the composite ten-thread campaign:

```sh
python3 experiments/field-regressions/run.py --suite optimization \
  --manifest experiments/field-regressions/repair_cases_arm_1.json \
  --out experiments/field-regressions/results/my-repair-arm --threads 1
python3 experiments/field-regressions/run_frozen.py \
  --from-run experiments/field-regressions/results/my-repair-arm \
  --manifest experiments/field-regressions/repair_cases_arm_10.json \
  --out experiments/field-regressions/results/my-repair-arm-10 --threads 10 \
  --families opt_ood,opt_ood_reuse,opt_pack,opt_packed_ood,opt_ntt
```

For an exact-binary replay, extract the archived `snapshot/` into a temporary
directory and pass `--binary <directory>/snapshot/field-regressions` to
`run_frozen.py`, with `--from-run` pointing to the recorded `repair-arm-02` run.
The runner verifies the executable hash. New hosts need their own confirmation;
the archived Mac measurements do not authorize production or x86 dispatch.
