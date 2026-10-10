# Falcon binder final production validation

All 100 final production/baseline processes verified; 165 release tests and six Python runner tests passed. Production was built without `bench-internals`. Falcon-512, batch 1024, security 128; workers 1/2/4/8/16; seeds 45–49; one warmup and five measured trials per process.

Timing uses one process median per seed, then paired geometric candidate/baseline ratios. The five seeds are the sampling units. Intervals enumerate all 5^5 paired bootstrap resamples and are descriptive 95% intervals. These seeds were previously used in V2; this campaign confirms the final production build but is not a fresh holdout for the transcript change.

| Production policy | Workers | Total ms, baseline → production | Total reduction [95% interval] | Prove-only reduction | Commit reduction | Verifier change | Lifetime RSS change |
|:--|--:|--:|--:|--:|--:|--:|--:|
| arithmetic / words | 1 | 282.93 → 181.91 | 35.63% [35.19, 35.96] | 35.88% | 33.75% | -0.60% | -11.89% |
| arithmetic / words | 2 | 176.10 → 105.42 | 40.10% [39.91, 40.22] | 40.80% | 34.21% | -31.90% | -12.45% |
| arithmetic / words | 4 | 110.57 → 64.81 | 41.29% [40.93, 41.65] | 42.03% | 33.78% | -40.73% | -12.22% |
| arithmetic / words | 8 | 59.96 → 45.95 | 23.19% [22.76, 23.62] | 22.69% | 29.63% | +0.56% | -0.12% |
| arithmetic / words | 16 | 75.18 → 49.86 | 33.77% [33.52, 33.98] | 34.45% | 21.62% | -47.21% | -12.45% |
| full / replay | 1 | 2000.49 → 1983.23 | 0.92% [0.85, 1.00] | 0.31% | 3.88% | +0.27% | +0.01% |
| full / replay | 2 | 1173.41 → 1072.86 | 8.40% [8.26, 8.53] | 9.27% | 3.62% | -1.02% | +0.05% |
| full / replay | 4 | 669.02 → 628.12 | 6.16% [5.94, 6.42] | 6.76% | 3.53% | -1.02% | -2.85% |
| full / replay | 8 | 429.70 → 404.93 | 5.94% [5.59, 6.34] | 6.57% | 2.52% | -1.04% | +0.07% |
| full / replay | 16 | 348.75 → 336.15 | 3.65% [3.32, 4.21] | 4.14% | 1.84% | -1.38% | +0.02% |

Positive reductions mean faster; positive verifier/RSS changes mean more cost. Timing columns are separate medians, so their values need not sum exactly. Baseline comparisons combine the binder policy, the zipped integer-product base-case loop, and coalescing tiny Blake3 absorption writes while preserving transcript framing; they do not isolate one optimization.

| Variant | Workers | Production total ms | Production commit ms | Production prove ms | Production verify ms | Production peak RSS MiB |
|:--|--:|--:|--:|--:|--:|--:|
| arithmetic | 1 | 181.911 | 22.793 | 159.083 | 44.270 | 110.64 |
| arithmetic | 2 | 105.424 | 11.857 | 93.453 | 30.766 | 113.71 |
| arithmetic | 4 | 64.812 | 6.485 | 58.225 | 23.863 | 115.20 |
| arithmetic | 8 | 45.954 | 3.869 | 41.991 | 20.262 | 117.16 |
| arithmetic | 16 | 49.856 | 3.142 | 46.805 | 19.384 | 120.45 |
| full | 1 | 1983.235 | 326.631 | 1656.149 | 45.444 | 2298.70 |
| full | 2 | 1072.862 | 177.567 | 895.295 | 38.421 | 2308.12 |
| full | 4 | 628.117 | 105.996 | 520.314 | 34.997 | 2251.67 |
| full | 8 | 404.931 | 73.874 | 331.642 | 33.227 | 2309.75 |
| full | 16 | 336.149 | 61.352 | 273.982 | 32.233 | 2319.05 |

Among the requested worker counts on this pinned host and workload, the lowest observed production total-latency median is arithmetic at 8 workers (45.95 ms) and full at 16 workers (336.15 ms). This is an observed ordering for this workload, not a recommendation for other hosts or shapes.

| Variant | Workers | Verifier ratio [95% interval] | RSS ratio [95% interval] |
|:--|--:|--:|--:|
| arithmetic | 1 | 0.9940 [0.9889, 0.9992] (pass) | 0.8811 [0.8744, 0.8885] (pass) |
| arithmetic | 2 | 0.6810 [0.6758, 0.6887] (pass) | 0.8755 [0.8740, 0.8768] (pass) |
| arithmetic | 4 | 0.5927 [0.5896, 0.5958] (pass) | 0.8778 [0.8766, 0.8790] (pass) |
| arithmetic | 8 | 1.0056 [0.9947, 1.0156] (inconclusive) | 0.9988 [0.9954, 1.0022] (inconclusive) |
| arithmetic | 16 | 0.5279 [0.5268, 0.5288] (pass) | 0.8755 [0.8740, 0.8767] (pass) |
| full | 1 | 1.0027 [0.9907, 1.0153] (inconclusive) | 1.0001 [0.9998, 1.0003] (inconclusive) |
| full | 2 | 0.9898 [0.9886, 0.9911] (pass) | 1.0005 [0.9991, 1.0027] (inconclusive) |
| full | 4 | 0.9898 [0.9836, 0.9941] (pass) | 0.9715 [0.9634, 0.9796] (pass) |
| full | 8 | 0.9896 [0.9873, 0.9919] (pass) | 1.0007 [0.9968, 1.0041] (inconclusive) |
| full | 16 | 0.9862 [0.9803, 0.9911] (pass) | 1.0002 [0.9983, 1.0016] (inconclusive) |

The gate labels only classify these descriptive intervals against exact parity: `pass` means the upper endpoint is at most 1, `regression` means the lower endpoint is above 1, and `inconclusive` means parity lies in the interval. They are not universal acceptance guarantees.

The earlier V2 eight-worker arithmetic verifier increase is reduced from +3.10% to +0.56%, but the final interval crosses parity, so its removal is unresolved. The one-worker arithmetic estimate changes from the earlier +0.65% to −0.60%; its exact-bootstrap interval is below parity, while the paired-log t sensitivity interval [0.9859, 1.0023] crosses parity. That conclusion is method-sensitive with five seeds. Full one-worker verification has a +0.27% point estimate and an interval crossing parity. No all-metric nonregression claim is supported.

RSS uses the maximum high-water observation across all six trials in each process, including warmup. The median sampled high-water value would differ in 87/100 processes. Peaks include fixture setup, allocator retention, and proof-digest formatting; they do not measure isolated binder allocation or establish long-lived steady-state memory.

| Seed | Baseline proof bytes | Word proof bytes | Difference |
|--:|--:|--:|--:|
| 45 | 262402 | 262762 | +360 |
| 46 | 264194 | 263690 | -504 |
| 47 | 264802 | 261706 | -3096 |
| 48 | 263042 | 262762 | -280 |
| 49 | 263746 | 263946 | +200 |

Complete full proof payloads are unchanged. Word payloads are identical across worker counts and unchanged from V2 within the word protocol, but exceed the bit baseline for seeds 45, 49. Mean word payload change is -664 bytes. Complete word proof-payload nonregression is not supported by these fixtures.

The cross-campaign audit covers 480 processes and 2320 trials from screen, pilot-v2, holdout-v2, and production. Raw stdout hashes, trial counts, verification flags, public shape/security/seed, CPU affinity, zero exit status, and absent stage tracing all pass. Input/source commitments agree across protocols; complete proof digests and sizes agree across workers, kernels, and revisions within each observed protocol. Protocols are classified using the emitted ID because production selects words with `auto`.

Production defaults select the word protocol for public N=512, batch=1024, security=128 independently of worker count. The word protocol uses its v6 domain; the explicit Bits API retains v5. Kernel dispatch is qualified at workers 1/2/4/8/16. Exact-byte transcript preservation is supported by the cross-revision proof audit and the release framing tests; these checks do not replace protocol soundness review.

Five seeds on one host limit generalization. Intervals are not corrected for multiple comparisons. Frozen V2 and final production must remain separate evidence records; final improvements cannot all be attributed to the binder.

The JSON file retains every paired seed ratio, raw process statistic, descriptive interval, sensitivity interval, test-suite result, identity audit count, and frozen binary hash. The CSV is a compact 10-row extraction.
