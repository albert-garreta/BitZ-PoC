# Combined Falcon preparation comparison

Medians of five measured trials after one warm-up. All 240 proofs verified. Inputs and proof payload breakdowns match between variants and across thread counts.

| Batch | Security | Threads | Preparation before/after (ms) | Preparation reduction | Total before/after (ms) | Total reduction |
|---:|---:|---:|---:|---:|---:|---:|
| 32 | 100 | 1 | 8.174 / 5.317 | 35.0% | 84.707 / 82.639 | 2.4% |
| 32 | 100 | 2 | 4.925 / 3.540 | 28.1% | 46.629 / 45.411 | 2.6% |
| 32 | 100 | 4 | 3.738 / 2.937 | 21.4% | 28.341 / 27.453 | 3.1% |
| 32 | 100 | 8 | 2.732 / 2.347 | 14.1% | 18.978 / 18.461 | 2.7% |
| 32 | 100 | 16 | 2.491 / 2.259 | 9.3% | 17.781 / 17.518 | 1.5% |
| 32 | 128 | 1 | 8.164 / 5.310 | 35.0% | 102.820 / 100.251 | 2.5% |
| 32 | 128 | 2 | 4.919 / 3.480 | 29.3% | 56.454 / 55.063 | 2.5% |
| 32 | 128 | 4 | 3.459 / 2.988 | 13.6% | 33.929 / 33.534 | 1.2% |
| 32 | 128 | 8 | 2.796 / 2.467 | 11.8% | 22.550 / 22.064 | 2.2% |
| 32 | 128 | 16 | 2.606 / 2.331 | 10.6% | 20.923 / 20.524 | 1.9% |
| 1024 | 100 | 1 | 256.410 / 163.979 | 36.0% | 2420.931 / 2357.097 | 2.6% |
| 1024 | 100 | 2 | 158.427 / 112.877 | 28.8% | 1339.684 / 1285.966 | 4.0% |
| 1024 | 100 | 4 | 110.385 / 87.027 | 21.2% | 793.436 / 764.267 | 3.7% |
| 1024 | 100 | 8 | 85.724 / 74.223 | 13.4% | 517.074 / 505.507 | 2.2% |
| 1024 | 100 | 16 | 73.714 / 67.905 | 7.9% | 398.692 / 393.000 | 1.4% |
| 1024 | 128 | 1 | 256.073 / 164.739 | 35.7% | 2502.011 / 2406.813 | 3.8% |
| 1024 | 128 | 2 | 157.159 / 112.114 | 28.7% | 1382.798 / 1324.410 | 4.2% |
| 1024 | 128 | 4 | 109.572 / 86.223 | 21.3% | 811.426 / 784.178 | 3.4% |
| 1024 | 128 | 8 | 84.873 / 73.838 | 13.0% | 528.753 / 516.182 | 2.4% |
| 1024 | 128 | 16 | 73.279 / 67.359 | 8.1% | 410.118 / 400.671 | 2.3% |

Preparation includes derivation, validation, packing, and the initial commitment. Baseline phases are added per sample before taking the median. Total prover also includes proof generation. Setup, input generation, public-target hashing, and input copies are excluded from both variants.

Runs are sequential, with adjacent baseline/candidate configurations and alternating first variant. Workers use physical CPUs 0 through N−1, with main and auxiliary workers on CPU 0. Sixteen threads spans both CPU chiplets. Verification uses the configured pool. RSS includes input preparation and caches. Raw trials, commands, source hashes, and binary hashes are retained.
