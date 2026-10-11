# Falcon commitment experiment measurements

Batch 1024, security 128, seed 42; two warm-ups and three measured trials per fresh process. Times are milliseconds and absolute values are medians of process medians. Changes use the geometric mean of paired candidate/baseline ratios; brackets show a paired bootstrap 95% interval. Intervals are exploratory and unadjusted for multiple experiments. These measurements do not establish MacBook performance.

| Experiment | Workload | Threads | Pairs | Commit, ms | Commit change [95%] | Total, ms | Total change [95%] | Prove change | Verify change | RSS change [95%] | 1% latency gates |
|---|---|---:|---:|---|---|---|---|---:|---:|---|---|
| arithmetic | full 512 | 1 | 5 | 309.87 → 303.89 | -1.88% [-1.95, -1.81] | 2601.49 → 2592.77 | -0.35% [-0.41, -0.30] | -0.17% | -14.49% | +0.00% [-0.01, +0.02] | pass/pass/pass/pass |
| arithmetic | full 512 | 8 | 10 | 72.24 → 67.65 | -6.25% [-6.93, -5.54] | 480.85 → 468.90 | -2.39% [-2.62, -2.16] | -1.66% | -20.06% | -0.02% [-0.15, +0.12] | pass/pass/pass/pass |
| arithmetic | full 1024 | 1 | 5 | 551.68 → 540.34 | -2.09% [-2.32, -1.87] | 4635.75 → 4605.71 | -0.63% [-0.74, -0.53] | -0.44% | -16.95% | +0.02% [-0.00, +0.04] | pass/pass/pass/pass |
| arithmetic | full 1024 | 8 | 5 | 134.57 → 125.47 | -6.75% [-7.27, -6.13] | 909.51 → 886.57 | -2.51% [-2.60, -2.41] | -1.78% | -24.79% | +0.05% [-0.01, +0.11] | pass/pass/pass/pass |
| encoding | full 512 | 1 | 5 | 310.72 → 310.15 | -0.14% [-0.42, +0.07] | 2608.28 → 2603.41 | -0.14% [-0.27, -0.01] | -0.15% | +0.37% | -0.07% [-0.20, +0.01] | pass/pass/pass/pass |
| encoding | full 512 | 8 | 5 | 72.63 → 74.74 | +2.92% [+2.14, +3.53] | 479.05 → 481.70 | +0.39% [+0.18, +0.63] | -0.00% | +0.26% | +0.00% [-0.05, +0.06] | regression/pass/pass/pass |
| encoding | full 1024 | 1 | 5 | 552.31 → 554.68 | +0.42% [+0.35, +0.50] | 4636.39 → 4640.12 | +0.02% [-0.06, +0.08] | -0.04% | +0.67% | -0.00% [-0.01, +0.01] | pass/pass/pass/pass |
| encoding | full 1024 | 8 | 5 | 134.05 → 135.58 | +0.50% [-1.05, +1.56] | 910.27 → 913.56 | +0.22% [+0.09, +0.35] | +0.25% | +0.58% | +0.03% [-0.01, +0.10] | inconclusive/pass/pass/inconclusive |
| final | arithmetic 512 | 1 | 10 | 9.49 → 9.51 | +0.20% [-0.02, +0.41] | 300.58 → 301.02 | +0.20% [+0.09, +0.31] | +0.20% | +0.22% | -0.02% [-0.08, +0.03] | pass/pass/pass/pass |
| final | arithmetic 512 | 8 | 10 | 2.42 → 2.43 | +0.62% [-0.76, +2.05] | 58.64 → 58.69 | +0.11% [-0.07, +0.31] | +0.19% | +0.51% | +0.02% [-0.19, +0.22] | inconclusive/pass/pass/inconclusive |
| final | arithmetic 1024 | 1 | 5 | 19.49 → 19.50 | +0.09% [-0.24, +0.45] | 561.00 → 561.54 | +0.21% [-0.15, +0.72] | +0.21% | +0.07% | -0.00% [-0.07, +0.06] | pass/pass/pass/pass |
| final | arithmetic 1024 | 8 | 10 | 4.67 → 4.64 | -0.01% [-1.67, +1.73] | 109.94 → 109.82 | -0.11% [-0.24, +0.02] | -0.20% | -0.94% | +0.02% [-0.11, +0.15] | inconclusive/pass/pass/pass |
| keccak | full 512 | 1 | 5 | 310.30 → 306.23 | -1.25% [-1.46, -1.03] | 2602.60 → 2599.25 | -0.14% [-0.26, -0.05] | +0.01% | +0.49% | +1.14% [+1.13, +1.14] | pass/pass/pass/pass |
| keccak | full 512 | 8 | 5 | 71.80 → 73.69 | +2.67% [+1.79, +3.57] | 478.22 → 480.38 | +0.32% [+0.10, +0.52] | -0.08% | +0.59% | +1.07% [+1.00, +1.14] | regression/pass/pass/pass |
| keccak | full 1024 | 1 | 5 | 552.21 → 542.24 | -1.83% [-1.96, -1.70] | 4635.05 → 4616.71 | -0.34% [-0.45, -0.21] | -0.14% | +0.26% | +2.63% [+2.61, +2.65] | pass/pass/pass/pass |
| keccak | full 1024 | 8 | 5 | 135.65 → 139.05 | +2.70% [+1.93, +3.50] | 909.71 → 908.25 | -0.16% [-0.44, +0.13] | -0.68% | +0.24% | +2.73% [+2.68, +2.76] | regression/pass/pass/pass |
| merkle | full 512 | 1 | 5 | 309.77 → 311.19 | +0.42% [+0.25, +0.57] | 2599.55 → 2600.93 | +0.02% [-0.07, +0.10] | -0.03% | +0.33% | -0.07% [-0.20, +0.01] | pass/pass/pass/pass |
| merkle | full 512 | 8 | 5 | 72.26 → 71.95 | -0.11% [-0.66, +0.61] | 479.96 → 478.04 | -0.23% [-0.47, +0.04] | -0.29% | +0.11% | -0.09% [-0.25, +0.00] | pass/pass/pass/pass |
| merkle | full 1024 | 1 | 5 | 552.50 → 554.20 | +0.31% [+0.21, +0.37] | 4631.28 → 4624.71 | -0.22% [-0.27, -0.17] | -0.28% | +0.44% | +0.02% [+0.00, +0.03] | pass/pass/pass/pass |
| merkle | full 1024 | 8 | 5 | 135.04 → 135.25 | -0.18% [-1.01, +0.74] | 909.22 → 909.51 | +0.21% [-0.05, +0.49] | +0.23% | +0.35% | +0.01% [-0.07, +0.08] | pass/pass/pass/pass |

The 1% latency gates are listed in commit/total/prove/verify order: pass means upper95 ratio ≤ 1.01; regression means lower95 ratio > 1.01; otherwise inconclusive. These tolerance gates do not prove zero regression. Significant slowdowns below the tolerance and all metric intervals remain explicit in summary.json.

Separate instrumented runs explain stage costs; they do not enter performance selection. SHAKE includes fused witness packing and auxiliary construction. Source construction/packing are later arithmetic preparation. PCS is encoding plus Merkle; all component sums are formed per trial before medians. Other time is commit-body wall time minus the six disjoint scopes and includes logging overhead.

| Experiment | N | Threads | Pairs | SHAKE | Arithmetic | Source construction | Source packing | Encode | Merkle | Other |
|---|---:|---:|---:|---|---|---|---|---|---|---|
| arithmetic | 512 | 1 | 2 | 33.70 → 32.75 | 7.85 → 7.85 | 8.82 → 8.01 | 1.17 → 1.13 | 207.65 → 207.70 | 46.21 → 46.47 | 4.93 → 0.25 |
| arithmetic | 512 | 8 | 2 | 16.08 → 16.16 | 1.11 → 1.10 | 1.50 → 1.38 | 0.78 → 1.16 | 39.72 → 39.93 | 7.93 → 7.95 | 5.01 → 0.26 |
| arithmetic | 1024 | 1 | 2 | 65.23 → 63.59 | 15.32 → 15.15 | 16.54 → 15.02 | 2.07 → 2.24 | 350.98 → 351.67 | 91.67 → 92.27 | 9.83 → 0.44 |
| arithmetic | 1024 | 8 | 2 | 29.02 → 28.87 | 2.09 → 2.08 | 2.83 → 2.71 | 0.74 → 0.74 | 74.41 → 74.46 | 15.20 → 15.11 | 9.98 → 0.46 |
| encoding | 512 | 1 | 2 | 33.37 → 34.28 | 7.86 → 7.88 | 8.79 → 8.81 | 1.16 → 1.14 | 207.45 → 207.52 | 46.30 → 46.47 | 4.92 → 4.92 |
| encoding | 512 | 8 | 2 | 15.83 → 15.80 | 1.11 → 1.11 | 1.49 → 1.47 | 1.19 → 0.78 | 39.91 → 42.47 | 7.99 → 8.15 | 5.01 → 5.02 |
| encoding | 1024 | 1 | 2 | 65.56 → 66.20 | 15.25 → 15.22 | 17.40 → 16.52 | 2.09 → 2.27 | 351.29 → 352.38 | 92.07 → 92.14 | 9.82 → 9.82 |
| encoding | 1024 | 8 | 2 | 29.71 → 29.15 | 2.10 → 2.08 | 2.80 → 2.79 | 0.77 → 0.77 | 74.45 → 75.73 | 15.23 → 15.81 | 9.97 → 9.97 |
| keccak | 512 | 1 | 2 | 33.68 → 30.13 | 7.86 → 7.82 | 8.78 → 8.77 | 1.15 → 0.31 | 207.57 → 207.54 | 46.16 → 46.46 | 4.92 → 4.94 |
| keccak | 512 | 8 | 2 | 15.90 → 18.29 | 1.10 → 1.11 | 1.47 → 1.50 | 0.78 → 0.34 | 39.71 → 39.72 | 7.87 → 7.97 | 5.01 → 5.03 |
| keccak | 1024 | 1 | 2 | 65.87 → 56.88 | 15.35 → 15.25 | 16.58 → 16.71 | 2.07 → 2.32 | 350.30 → 351.25 | 91.80 → 91.22 | 9.81 → 9.74 |
| keccak | 1024 | 8 | 2 | 28.82 → 32.00 | 2.10 → 2.08 | 2.79 → 2.79 | 0.74 → 0.75 | 74.51 → 73.93 | 15.27 → 15.13 | 9.95 → 9.87 |
| merkle | 512 | 1 | 2 | 33.68 → 34.27 | 7.85 → 7.80 | 8.76 → 8.93 | 1.14 → 1.12 | 207.43 → 208.35 | 46.32 → 46.98 | 4.92 → 4.92 |
| merkle | 512 | 8 | 2 | 15.76 → 15.86 | 1.10 → 1.11 | 1.48 → 1.52 | 0.76 → 1.19 | 40.02 → 39.88 | 8.01 → 7.91 | 5.00 → 5.01 |
| merkle | 1024 | 1 | 2 | 64.70 → 65.86 | 15.47 → 15.25 | 16.52 → 16.80 | 2.07 → 2.09 | 350.58 → 352.05 | 91.68 → 92.08 | 9.80 → 9.73 |
| merkle | 1024 | 8 | 2 | 29.04 → 29.63 | 2.11 → 2.10 | 2.87 → 2.92 | 0.77 → 0.76 | 74.18 → 74.51 | 15.29 → 15.27 | 9.95 → 9.85 |

Arithmetic diagnostics measure only `falcon_algebraic:witness`, excluding packing and PCS.

| Experiment | N | Threads | Pairs | Arithmetic generation, ms | Change [95%], descriptive |
|---|---:|---:|---:|---|---|
| final | 512 | 1 | 2 | 6.65 → 6.66 | +0.22% [+0.19, +0.26] |
| final | 512 | 8 | 2 | 1.40 → 1.38 | -1.26% [-9.94, +8.26] |
| final | 1024 | 1 | 2 | 13.52 → 13.48 | -0.23% [-0.40, -0.06] |
| final | 1024 | 8 | 2 | 2.59 → 2.62 | +1.04% [-1.54, +3.68] |

Validated 320 processes and 1600 proofs across 13 completed campaigns. Inputs, source roots, proof debug digests, payloads, capacity and public protocol/source shape match within each mode and configuration across all included campaigns. The debug digest is an exact-build comparison aid, not a stable proof encoding. RSS is whole-process peak memory.

All completed extension campaigns within an experiment are retained and pooled when their builds and variant environments match. Detailed stage samples, paired intervals, source manifests and selection evidence flags are listed in summary.json; an inconclusive interval does not establish nonregression.
