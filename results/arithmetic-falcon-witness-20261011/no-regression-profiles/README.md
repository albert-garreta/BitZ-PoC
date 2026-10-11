# Final compact candidate versus original v5 baseline: diagnostic profiles

All 16 degree/security/batch/thread configurations were profiled sequentially with seed 42, one warmup and three measured verified proofs. The 64 candidate rows exactly match the preserved final-candidate source roots, proof Debug digests and payload sizes. Both variants used native release builds and pinned workers; no builds/tests ran concurrently.

These are diagnostic span times, not replacement latency measurements. Entries below are candidate minus baseline in milliseconds, using medians of the three measured trials. Negative means less time. The `Other` bucket is total proving time minus ring, arithmetic, BitZ bridge and PCS spans; it includes joint sumcheck, compact claim preparation and protocol setup. Its attribution is not exclusively joint sumcheck. Nested spans overlap: do not sum `Joint setup` with `Other`.

| N | Security | Batch | Threads | Packing/commit Δ | Ring Δ | Arithmetic Δ | Bridge Δ | PCS Δ | Other Δ | Joint setup Δ |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 32 | 1 | +0.014 | -0.017 | +0.024 | +0.061 | -0.029 | +0.223 | +0.093 |
| 512 | 100 | 32 | 8 | +0.030 | -0.043 | +0.052 | -0.000 | -0.043 | +0.332 | +0.121 |
| 512 | 100 | 1024 | 1 | +1.556 | +0.452 | -3.755 | +0.046 | -4.695 | -1.984 | -0.898 |
| 512 | 100 | 1024 | 8 | +0.705 | -0.067 | -0.010 | +0.031 | -0.471 | +0.311 | +0.420 |
| 512 | 128 | 32 | 1 | +0.017 | -0.026 | +0.035 | +0.445 | -0.206 | +0.093 | +0.092 |
| 512 | 128 | 32 | 8 | +0.019 | +0.003 | +0.029 | +0.452 | +0.014 | +0.204 | +0.121 |
| 512 | 128 | 1024 | 1 | +1.702 | +0.688 | -0.263 | -3.293 | -5.397 | -0.964 | -0.840 |
| 512 | 128 | 1024 | 8 | +0.796 | -0.094 | -0.241 | -0.176 | -0.535 | +0.931 | +0.420 |
| 1024 | 100 | 32 | 1 | +0.004 | -0.046 | -0.045 | +0.103 | -0.514 | -0.233 | -0.056 |
| 1024 | 100 | 32 | 8 | +0.025 | -0.084 | -0.124 | +0.027 | -0.193 | -0.012 | +0.042 |
| 1024 | 100 | 1024 | 1 | +1.575 | +1.298 | -2.730 | +0.200 | -9.426 | -2.301 | -1.992 |
| 1024 | 100 | 1024 | 8 | +1.868 | -0.425 | +0.038 | +0.374 | -0.850 | -1.336 | -0.001 |
| 1024 | 128 | 32 | 1 | +0.007 | -0.049 | +0.422 | -0.046 | -0.674 | -0.146 | -0.058 |
| 1024 | 128 | 32 | 8 | +0.020 | -0.106 | +0.083 | +0.042 | -0.234 | +0.083 | +0.045 |
| 1024 | 128 | 1024 | 1 | +1.674 | +1.009 | +1.385 | -0.020 | -13.680 | -3.833 | -1.969 |
| 1024 | 128 | 1024 | 8 | +1.543 | -0.858 | -0.080 | +0.210 | -1.538 | +1.004 | -0.011 |

For Falcon-512 batch 32, joint packed setup rises from approximately 0.132 to 0.224 ms with one thread and 0.063 to 0.184 ms with eight threads. Packing/commit rises only 0.01–0.03 ms. This is consistent with overhead from the two dense compact-plane tensor passes at the unchanged minimum PCS allocation.

For Falcon-512 batch 1024 with eight threads, packing/commit adds 0.70–0.80 ms and other/joint work adds 0.31–0.93 ms, while PCS saves approximately 0.47–0.54 ms. Ring time is approximately unchanged. The remaining multithread regression therefore differs from the earlier scalar ring-recurrence regression.

At security 128, the transcript changes between v5 and v6 can also change deterministic grinding work for this single seed. Component timing differences in unchanged bridge/arithmetic code should not automatically be attributed to the compact decoder.

`component-diffs.json` contains baseline/candidate/difference values for every listed component. `stage-summary.json` contains all available stage medians, separated into commit, prove and verify operations. Run commands, raw traces, proof identity checks and machine metadata are retained alongside these summaries.
