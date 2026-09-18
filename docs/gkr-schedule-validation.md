# Measured GKR scheduling

The automatic policy uses public forest geometry, claim path, and the Rayon
worker count. It performs no calibration while proving. Explicit `l2`, `l4`, and
`l8` requests override it. Multi-claim L2 and L8 at depth below five are rejected;
they are not silently replaced by another schedule.

Let `d = row_vars + log2(physical_word_bits)`, `s = col_vars`, and `n = d + s`.
The rules are evaluated in order:

| Condition | Schedule |
|---|---|
| macOS AArch64, single-claim, one worker, `d = 13`, `12 <= s <= 14` | L2 |
| `d >= 13`, `n >= 25`, and either more than four workers or `d <= s + 1` | L8 |
| Single-claim, at most four workers, and `d >= s` | L2 |
| Remaining geometries and paths | L4 |

L4 provides the fallback for shapes without a demonstrated advantage from
another schedule. The low-worker rule never selects the unimplemented
multi-claim L2 path. The large-forest rule includes the measured multiswap
shapes. Small, wide SHA forests stay on L4. These thresholds describe this
measurement campaign; they are not a promise that any schedule is fastest on
every processor or input shape.

The Apple Silicon exception comes from the MultiSwap recovery investigation
on an M1 Max. The original AMD-qualified L8 choice added a PCS opening cost at
one worker. Alternating diagnostic processes (one warmup and five verified
proofs each) compared all three schedules: two process blocks for batches 1
and 4, and three for batch 2. L2 was the fastest at all three batch counts and
also used less peak RSS than L4 in these diagnostics. The L2 choice retains
more intermediate values than L8 and increase peak RSS, so explicit L8 remains
available for memory-constrained runs. Proof and transcript fingerprints were
identical across schedules.

The exception uses only public geometry, platform, and worker count. It does
not change the x86 policy, multi-claim policy, multi-worker cases, or explicit
schedule requests. Unmeasured Apple geometries retain the existing policy.
The raw diagnostics are retained in
`bench_results/multiswap-recovery-20260918/schedule-diagnostic*/`; fresh matched
confirmation against `ac0aa44c` and `c4866868` is retained separately under
`bench_results/multiswap-schedule-recovery-20260918/`. The original AMD
measurements below remain historical evidence for that platform.

## Measurement method

Measurements were collected on an AMD Ryzen 9 9950X3D, Linux x86-64, using Rust
1.98.1, `RUSTFLAGS='-C target-cpu=native'`, the Cargo bench profile, fat LTO,
and one codegen unit. Allocation instrumentation was disabled. The governor was
`powersave`; no machine settings were changed. Affinity used CPUs 0 through 7,
then 16 and 17 for ten-worker cases, within one shared L3 group.

Each configuration starts with six alternating process blocks, one warmup per
process, and five verified measured repetitions. Rust records raw trials and
phase totals. Python uses standard medians and Type-7 percentiles. Paired
intervals bootstrap the geometric mean of the six ratios of block medians.
Warmups are excluded from the ordinary latency statistics. These are per-case
95% intervals, not simultaneous guarantees across all tested cases.

Schedule ranking uses complete proving latency:

- Standalone multiplication: packing, commitment, and proving, with the witness
  already available.
- Multiplication comparison: witness generation through proof completion.
- SHA chain: packing, commitment, and proving.
- SHA/P-256: its recorded end-to-end prover boundary.
- Multiswap: its application prover boundary.

GKR timings explain the results but do not replace those boundaries. Memory
workers run separately from latency measurements. The full 24-block, 116-case
qualification gate remains available in `scripts/qualify_gkr.py`; this focused
merge campaign does not claim to have passed that gate.

## Sources and proof checks

The merge parents are `0a76b44418b379bd93c63fc356f38204d9ba19d2` and
`66d5ab28f21243a0ecb19cc1ea74a1944f327f32`. Each checkout used its own Cargo target
directory. Frozen executables, source patches, commands, stdout/stderr, and raw
campaigns are retained under `.tmp/merge-validation/` in the feature worktree.
The compact checked-in [measurements](gkr-schedule-measurements.json) retain
raw measured samples, geometries, executable hashes, and fingerprint checks.
Historical result files were not rewritten.

All initial measured proofs verified. The merged L2/L4/L8 runs agree on proof and
transcript fingerprints. Comparable master proofs also agree. The old standalone
U32 benchmark fingerprints the **verifier** transcript; the current benchmark
fingerprints the **prover** transcript, so those transcript hashes are not a
cross-revision equality check. Their proof hashes agree. The older feature-parent
binaries do not emit every transcript diagnostic, so missing diagnostics are
not treated as equality evidence.

Only current multiplication campaigns are supported by the maintained Python
loader. Parsing the two historical parents was confined to an ignored,
one-off measurement script. There are no maintained compatibility readers.

## Results and confirmation

The initial sweep contains 2,340 verified measured proofs. Eight additional
size/thread crossover cases add 720, for 3,060 total before fresh confirmation.
The eight-thread U64 ranking and the intermediate-size U32 L4/L8 ranking were
inconclusive in the first six blocks. No thread-specific exception is inferred
from those ties.

Times below are milliseconds, reported as medians of the six block medians.
“Feature parent” is `0a76b444`; “Master parent” is `66d5ab28`. A dash marks a
configuration that the historical parent did not support or an added crossover
case that compared only schedules.

| Case | Feature parent | Master parent | L2 | L4 | L8 |
|---|---:|---:|---:|---:|---:|
| u32-n15-w1-t1 | 63.50 | 60.03 | 57.26 | 59.95 | 63.05 |
| u32-n19-w1-t10 | 166.03 | 142.40 | 156.17 | 154.14 | 140.44 |
| u32-n15-w8-t1 | 59.91 | 55.93 | 53.20 | 56.06 | 59.07 |
| u32-n19-w8-t10 | 164.56 | 138.18 | 151.74 | 152.47 | 138.77 |
| u64-n15-t1 | 123.25 | 114.87 | 110.64 | 115.12 | 121.82 |
| u64-n19-t10 | 326.21 | 297.95 | 291.10 | 301.42 | 280.27 |
| u128-n15-t1 | 246.91 | 228.52 | 219.07 | 228.78 | 238.61 |
| u128-n17-t10 | 162.80 | 151.35 | 160.00 | 150.07 | 147.49 |
| u64-n15-w3-t10 | 133.00 | — | 128.79 | 126.99 | 129.20 |
| u128-n15-w8-t1 | 1179.29 | — | 1111.38 | 1142.74 | 1165.86 |
| sha-n7-t1 | 60.70 | 55.81 | 63.14 | 55.60 | 62.10 |
| sha-n12-t10 | 228.76 | 210.07 | 229.36 | 211.57 | 214.24 |
| p256-n4-t1 | 96.55 | 76.84 | 76.15 | 77.08 | 77.10 |
| p256-n7-t10 | 43.08 | 27.28 | 28.27 | 27.27 | 28.43 |
| multiswap-b1-t1 | 492.49 | 459.13 | 472.19 | 464.83 | 447.96 |
| multiswap-b4-t10 | 445.56 | 426.69 | 430.41 | 430.32 | 380.59 |
| u32-n15-w1-t10 | — | — | 13.12 | 13.12 | 13.51 |
| u32-n19-w1-t1 | — | — | 755.91 | 842.95 | 850.63 |
| u32-n17-w1-t10 | — | — | 43.28 | 39.47 | 39.28 |
| u32-n18-w1-t10 | — | — | 80.79 | 79.10 | 72.62 |
| u64-n15-w3-t8 | — | — | 131.51 | 126.34 | 128.81 |
| u64-n19-t8 | — | — | 290.52 | 295.32 | 293.44 |
| u32-n19-w1-t4 | — | — | 232.24 | 244.69 | 254.25 |
| u32-n19-w1-t2 | — | — | 415.02 | 426.17 | 442.40 |

The final automatic-policy executable was replayed in two fresh alternating
blocks with five measured repetitions for four representative cases. All 80
proofs verified, bringing the measured comparison total to 3,140. Their proof
fingerprints agree with master. The table uses the median of the two block
medians; negative changes mean faster proving.

| Case | Auto (ms) | Master (ms) | Change |
|---|---:|---:|---:|
| u32-n15-w1-t1 | 57.38 | 60.25 | -4.76% |
| u32-n19-w1-t10 | 141.90 | 140.88 | +0.72% |
| sha-n12-t10 | 210.62 | 211.99 | -0.64% |
| multiswap-b1-t1 | 441.68 | 458.66 | -3.70% |

The initial SHA n12 comparison suggested a small slowdown (paired 95% ratio
interval 1.0014–1.0201). The fresh run reverses that ranking, so a SHA regression
is not established. The final U32 n19 ten-worker run is about 0.7% slower than
master, whereas the initial six-block L8 comparison was faster. Treat this small
cross-revision difference as unresolved, not as evidence of universal speedup.
Two confirmation blocks are insufficient for a strong statistical conclusion.
No additional schedule exceptions were added for these uncertain differences.

## Final validation

- Optimized library suite: 514 passed, 5 ignored.
- Benchmark integration suite: 48 passed, 2 ignored, with the native Perfetto
  processor configured. The two security-sweep fixtures now explicitly allow
  unsupported combinations, and the quad protocol fixture explicitly selects
  its required L4 schedule.
- Python reporting/process tests: 69 passed. The comparison-tool and report
  smoke runs also passed.
- Both multiplication targets built as optimized executables. Their independent
  feature configurations passed `cargo check`; the instrumented heap feature
  configuration also passed compilation.
- The final F2Z proof campaign verified 62 supported cases and recorded 34 skips,
  covering all integer widths and BabyBear, W=1/3/8, both split settings,
  one/eight workers, and security profiles 100/128.
- Eight isolated RSS replays reproduced their latency configurations, including
  resolved schedules. Memory records omit proof fingerprints.
- Native comparisons verified 14 cases (4 skips); witness generation completed
  38 (10 skips); PCS verified 8; PIOP verified 5; outer kernels completed 24;
  bounds checks verified 2. An entirely unsupported sweep correctly failed
  preflight with no runnable cases.
- Tampering, explicit schedule errors, automatic resolution, sweep expansion,
  environment isolation, warmup exclusion, and duplicate/incomplete record
  rejection are covered by the passing suites.

The full qualification campaign was not run. Instrumented heap runtime was not
rerun after the merge; heap compilation and isolated RSS replay passed. Fresh
automatic-policy confirmation covers four cases rather than every initial
schedule-comparison case. Results support the default on the measured hardware
and leave the small timing differences above unresolved.
