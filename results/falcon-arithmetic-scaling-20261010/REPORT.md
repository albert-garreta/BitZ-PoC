# Falcon-512 arithmetic scaling

**The retained change fixes the measured 8-to-16-worker scaling reversal, with small arithmetic and verification tradeoffs.** At 16 workers, total proving time improves by 9.89% for arithmetic and 2.92% for full Falcon. Candidate arithmetic runs 3.53% faster at 16 workers than at 8. Strict nonregression was not achieved: arithmetic at 1–4 workers is approximately 0.3–0.5% slower, and some verification measurements also increase.

The retained source is the original cyclic scheduling candidate above optimized revision `78c164f0fdb5939924561e2831241d7eb558c17b`. Its current source hashes exactly match `cyclic-source-manifest.json`; the retained native binaries are `cyclic-arithmetic` and `cyclic-full`. The later early shape-gate and inline experiments were rejected. This is not a direct comparison with the earlier `51b472c55`. Exact source and binary hashes are in [source-build.json](source-build.json).

## Retained holdout

Falcon-512, 1,024 signatures and 128-bit security. Seeds 60–64, one warmup plus five samples per process, randomized paired order 20261021, cached identical cases and standard physical-core affinity. All 80 processes and 480 trials verified. Actual protocol ID, input digest, source commitment and complete proof-debug digest match across configurations and worker counts for each variant and seed.

Times are medians of five process medians. Changes are geometric means of matched candidate/baseline ratios, so they need not equal ratios of displayed medians. Negative changes mean faster or less memory.

| Variant | Workers | Baseline total ms | Candidate total ms | Total change | 95% interval | Prove-only change | Verify change | Peak RSS change |
|---|---:|---:|---:|---:|---|---:|---:|---:|
| Arithmetic | 1 | 181.093 | 181.351 | +0.303% | [+0.100%, +0.445%] | +0.337% | +0.200% | +0.131% |
| Arithmetic | 4 | 64.899 | 65.282 | +0.526% | [+0.105%, +0.948%] | +0.263% | -0.179% | +0.260% |
| Arithmetic | 8 | 45.961 | 46.071 | +0.218% | [-0.783%, +1.247%] | +0.267% | -0.488% | +0.003% |
| Arithmetic | 16 | 49.469 | 44.551 | -9.891% | [-10.241%, -9.400%] | -10.840% | -1.150% | +0.095% |
| Full Falcon | 1 | 1995.938 | 1997.601 | +0.063% | [-0.056%, +0.168%] | +0.074% | -0.562% | -0.037% |
| Full Falcon | 4 | 630.993 | 630.858 | +0.035% | [-0.181%, +0.267%] | +0.027% | +0.503% | -0.283% |
| Full Falcon | 8 | 403.945 | 406.710 | +0.151% | [-0.335%, +0.639%] | +0.173% | +0.290% | -0.116% |
| Full Falcon | 16 | 335.292 | 325.378 | -2.920% | [-3.414%, -2.492%] | -3.623% | +0.460% | +0.005% |

Candidate arithmetic totals are **46.071 ms at 8 workers** and **44.551 ms at 16 workers**. The matched 16/8 change is **-3.535%**, with an exact 95% interval of **[-4.274%, -2.880%]**.

## Replication and remaining regressions

The unchanged binaries were independently replicated at 1 and 4 arithmetic workers using seeds 65–69, two warmups plus nine samples, and order 20261022. All 20 processes and 220 trials verified. At 1 worker, total time increased 0.475% and prove-only time 0.545%, both with positive confidence bounds. At 4 workers, total time increased 0.350% with an interval crossing zero; prove-only increased 0.433% and verification 0.708%, both with positive bounds.

Every retained holdout or replication metric with a positive lower confidence bound is listed below. These measured increases remain part of the result, even when small.

| Campaign | Variant | Workers | Metric | Change | 95% interval |
|---|---|---:|---|---:|---|
| Holdout | Arithmetic | 1 | Total proving | +0.303% | [+0.100%, +0.445%] |
| Holdout | Arithmetic | 1 | Prove only | +0.337% | [+0.114%, +0.494%] |
| Holdout | Arithmetic | 4 | Total proving | +0.526% | [+0.105%, +0.948%] |
| Holdout | Arithmetic | 4 | Prove only | +0.263% | [+0.102%, +0.421%] |
| Holdout | Full Falcon | 4 | Verification | +0.503% | [+0.116%, +0.913%] |
| Holdout | Full Falcon | 8 | Verification | +0.290% | [+0.003%, +0.673%] |
| Holdout | Full Falcon | 16 | Verification | +0.460% | [+0.043%, +0.985%] |
| Replication | Arithmetic | 1 | Total proving | +0.475% | [+0.185%, +0.679%] |
| Replication | Arithmetic | 1 | Prove only | +0.545% | [+0.207%, +0.768%] |
| Replication | Arithmetic | 4 | Prove only | +0.433% | [+0.110%, +0.771%] |
| Replication | Arithmetic | 4 | Verification | +0.708% | [+0.325%, +1.122%] |

No retained peak-RSS cell has a positive lower confidence bound. That does not guarantee unchanged memory. Full metric intervals and process medians, including inconclusive cells, are in [summaries.json](summaries.json).

## Change and diagnosis

Large forest rows retain a consistent Rayon worker across high-bit folds: row `y` belongs to worker `y % workers`. Dispatch requires a power-of-two pool above 8 workers, a divisible row count at least as large as the pool, and at least 4 MiB of touched matrix data. Large arenas and tables remain caller-allocated. Field operations, transcript messages, proof contents and verifier source are unchanged.

The Ryzen 9 9950X3D has 96 MiB of L3 shared by physical cores 0–7 and 32 MiB shared by cores 8–15. Standard 8-worker affinity stays in the first group; 16 workers span both. Tracing localized the penalty to forest JIT writes and dense rounds. The inclusive forest span increased from 14.997 to 22.208 ms, while integer column folds stayed near 0.382–0.390 ms. Nested trace spans overlap and are diagnostic only. The separate SMT-affinity experiment changed both physical-core count and locality, so it does not isolate cache causation.

## Rejected experiments

| Experiment | Measured result | Decision |
|---|---|---|
| Serial dense cutoff | Arithmetic total +1.11% at 8 workers; +0.46% at 16 | Reverted |
| Whole-forest worker entry | Total −1.77% at 16; peak RSS +46.14% | Reverted for memory cost |
| Coarse writes plus worker entry | Total −0.29% at 16; peak RSS +48.40% | Reverted; does not isolate coarse writes |
| Four wrapper inline hints | Arithmetic total +0.49%, +1.45%, +1.47% at 1/4/8 | Reverted |
| Wrapper plus backend inline hints | Arithmetic total +0.49%, +0.59%, +1.13% at 1/4/8 | Reverted |
| Early shape gate, later 720-trial holdout | Arithmetic total +0.62%, +1.38%, +1.27% at 1/4/8 | Reverted to the original cyclic constructor |

All rejected source changes were removed. Their complete comparisons, intervals, frozen binary hashes and raw audit references are folded into [summaries.json](summaries.json). The later `final-*` filenames identify the rejected shape-gate experiment and do not identify the retained build. Retaining an earlier measured candidate after later experiments is a selection decision; no third independent matrix was run after that selection. The retained source exactly matches its original freeze.

## Validation and limitations

The retained source passed 76 parallel BitZ tests, 72 tests without default features, and one grouped-prefix oracle in each feature mode. Development builds exercise generic arithmetic and dispatch oracles; the native production campaigns separately exercise the selected SIMD workload. The change does not alter intrinsic arithmetic or tails. Exact commands and log hashes are in [audit.json](audit.json); source review is in [review.md](review.md).

The audit checked source, frozen executable and raw-output hashes, complete recorded argv, environment settings, observed worker/auxiliary/verifier affinities, raw metric normalization, protocol/proof identities and non-overlapping timed processes. No builds, tests or tracing overlapped the retained timed campaigns. Case preparation runs are excluded from performance evidence.

Confidence intervals enumerate all 5^5 paired bootstrap resamples and are descriptive, with no adjustment for multiple cells. Five independent process pairs per cell limit precision. An interval crossing zero is inconclusive, not proof of nonregression. Peak RSS includes setup and proof-digest formatting. Results qualify this native build, workload and host; other processors and shapes may differ. The small arithmetic and verification tradeoffs remain unresolved.
