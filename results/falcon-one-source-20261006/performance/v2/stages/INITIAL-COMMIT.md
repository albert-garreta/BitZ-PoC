# Initial source commitment diagnostics for V2

Each value is one instrumented fresh-process run for seed 42 at 16 threads. These are diagnostics, not the statistical acceptance gate. Initial Merkle totals sum all three old source trees and compare them to the one joint tree.

| Protocol | Security | Batch | Old Merkle ms | Joint Merkle ms | Old full commit ms | Joint full commit ms |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 0.21 | 0.13 | 0.95 | 1.26 |
| native | 100 | 3 | 0.53 | 0.20 | 2.79 | 3.66 |
| native | 100 | 32 | 2.44 | 0.84 | 10.33 | 8.58 |
| native | 100 | 1024 | 42.98 | 13.46 | 166.92 | 142.83 |
| native | 128 | 1 | 0.24 | 0.10 | 1.12 | 1.07 |
| native | 128 | 3 | 0.47 | 0.21 | 2.32 | 3.11 |
| native | 128 | 32 | 1.97 | 0.76 | 8.38 | 7.21 |
| native | 128 | 1024 | 42.84 | 13.27 | 161.42 | 134.29 |
| shared-prime | 100 | 1 | 0.26 | 0.10 | 1.21 | 1.23 |
| shared-prime | 100 | 3 | 0.51 | 0.22 | 2.72 | 3.36 |
| shared-prime | 100 | 32 | 2.41 | 0.92 | 9.82 | 8.88 |
| shared-prime | 100 | 1024 | 42.99 | 13.30 | 162.67 | 140.86 |
| shared-prime | 128 | 1 | 0.23 | 0.10 | 1.11 | 1.20 |
| shared-prime | 128 | 3 | 0.49 | 0.20 | 2.75 | 3.66 |
| shared-prime | 128 | 32 | 2.13 | 0.87 | 9.55 | 8.29 |
| shared-prime | 128 | 1024 | 42.87 | 14.35 | 166.46 | 154.12 |

Candidate [joint-commit-timing] encoding includes allocation and replicate fill; baseline [commit-timing] ntt excludes them. Only Merkle timers and comparable enclosing phases are directly compared.

Nested tracing spans overlap. The JSON artifact retains each named span total for inspection; summing all spans would double-count time.
