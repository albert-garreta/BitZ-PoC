# Falcon witness generation at security 128

Exact ac2b42fea baseline versus the simplified NTT implementation at 5f2149a8f. Batch 1024, security target 128, seed 42, both Falcon degrees, one and eight threads, native CPU builds on Linux with an AMD Ryzen 9 9950X3D. These are not MacBook measurements.

Generation is measured in separate instrumented runs. Arithmetic generation includes checked polynomial reconstruction, centered S1, integer norms, slack and quotient values. Full generation sums the disjoint SHAKE and arithmetic scopes **for each trial before taking medians**. SHAKE generation includes its fused packed-witness construction and auxiliary values; arithmetic generation includes HashToPoint and the native ring/norm trace. Both exclude the later arithmetic-source packing and PCS commitment. The diagnostic baseline adds only matching SHAKE/arithmetic timing spans.

Witness + commit and total prover times below come from separate uninstrumented runs. Absolute values are medians of process medians. Percentage changes are geometric means of paired candidate/baseline ratios and cannot be calculated directly from those displayed medians.

| Workload | Threads | Generation, ms (before → after) | Generation change | Witness + commit, ms (before → after) | Total prover, ms (before → after) |
|---|---:|---:|---:|---:|---:|
| Arithmetic 512 | 1 | 32.197 → 6.719 | -79.13% | 34.893 → 9.512 | 324.816 → 299.652 |
| Arithmetic 512 | 8 | 4.582 → 1.315 | -71.31% | 5.589 → 2.324 | 61.713 → 58.714 |
| Arithmetic 1024 | 1 | 95.993 → 13.522 | -85.91% | 101.959 → 19.643 | 639.242 → 557.442 |
| Arithmetic 1024 | 8 | 13.149 → 2.534 | -80.74% | 15.019 → 4.638 | 119.196 → 109.392 |
| Full 512 | 1 | 68.587 → 48.678 | -29.03% | 336.787 → 315.709 | 2617.634 → 2593.885 |
| Full 512 | 8 | 19.640 → 16.790 | -14.53% | 75.482 → 72.268 | 477.798 → 474.161 |
| Full 1024 | 1 | 164.201 → 92.528 | -43.65% | 634.310 → 560.924 | 4692.000 → 4607.440 |
| Full 1024 | 8 | 42.102 → 30.921 | -26.55% | 146.438 → 135.095 | 912.043 → 900.718 |

Each uninstrumented cell has three alternating paired process blocks; each diagnostic cell has two. Every process has two warm-ups followed by three measured trials. These short runs are a focused comparison, not a broad performance qualification. Detailed paired ratios and descriptive bootstrap intervals, including prove, verify, RSS and the separate full-witness components, are retained in `summary.json`.

**Validation:** all four campaigns completed, 80 processes and 400 verified proofs. Input digest, source root, complete-proof debug digest, payload size/breakdown and capacity match across variants and campaigns for every workload/thread configuration. The debug digest is an exact-build comparison aid, not a stable proof wire encoding.
