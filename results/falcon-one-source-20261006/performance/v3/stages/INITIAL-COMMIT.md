# Initial source commitment diagnostics for V3

Each value is one instrumented fresh-process run for seed 42 at 16 threads. These are diagnostics, not the statistical acceptance gate. Initial Merkle totals sum all three old source trees and compare them to the one joint tree.

| Protocol | Security | Batch | Old Merkle ms | Joint Merkle ms | Old full commit ms | Joint full commit ms |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 0.27 | 0.12 | 1.33 | 1.55 |
| native | 100 | 3 | 0.51 | 0.21 | 2.57 | 2.69 |
| native | 100 | 32 | 2.41 | 0.92 | 10.53 | 8.30 |
| native | 100 | 1024 | 43.88 | 14.17 | 179.40 | 143.45 |
| native | 128 | 1 | 0.24 | 0.14 | 1.14 | 1.16 |
| native | 128 | 3 | 0.49 | 0.21 | 2.57 | 2.65 |
| native | 128 | 32 | 2.34 | 0.86 | 10.20 | 8.40 |
| native | 128 | 1024 | 44.17 | 13.53 | 173.66 | 139.17 |
| shared-prime | 100 | 1 | 0.26 | 0.10 | 1.17 | 1.04 |
| shared-prime | 100 | 3 | 0.50 | 0.20 | 2.73 | 2.96 |
| shared-prime | 100 | 32 | 2.30 | 0.86 | 10.13 | 8.38 |
| shared-prime | 100 | 1024 | 43.08 | 12.79 | 160.78 | 134.36 |
| shared-prime | 128 | 1 | 0.29 | 0.10 | 1.16 | 1.58 |
| shared-prime | 128 | 3 | 0.51 | 0.20 | 2.88 | 3.04 |
| shared-prime | 128 | 32 | 2.21 | 0.86 | 9.06 | 8.05 |
| shared-prime | 128 | 1024 | 43.59 | 13.40 | 172.37 | 143.90 |

Candidate [joint-commit-timing] encoding includes allocation and replicate fill; baseline [commit-timing] ntt excludes them. Only Merkle timers and comparable enclosing phases are directly compared.

Nested tracing spans overlap. The JSON artifact retains each named span total for inspection; summing all spans would double-count time.
