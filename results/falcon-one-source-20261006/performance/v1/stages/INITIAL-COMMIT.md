# Initial source commitment diagnostics

Each value is one instrumented fresh-process run for seed 42 at 16 threads. These are diagnostics, not the statistical acceptance gate. Initial Merkle totals sum all three old source trees and compare them to the one joint tree.

| Protocol | Security | Batch | Old Merkle ms | Joint Merkle ms | Old full commit ms | Joint full commit ms |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 0.27 | 0.13 | 1.19 | 1.22 |
| native | 100 | 3 | 0.47 | 0.21 | 2.84 | 3.09 |
| native | 100 | 32 | 2.32 | 0.90 | 10.18 | 7.86 |
| native | 100 | 1024 | 42.79 | 13.11 | 163.22 | 144.47 |
| native | 128 | 1 | 0.25 | 0.11 | 1.13 | 1.33 |
| native | 128 | 3 | 0.50 | 0.21 | 2.78 | 3.60 |
| native | 128 | 32 | 2.42 | 0.87 | 10.15 | 8.24 |
| native | 128 | 1024 | 43.45 | 13.31 | 170.37 | 138.45 |
| shared-prime | 100 | 1 | 0.25 | 0.10 | 1.13 | 1.33 |
| shared-prime | 100 | 3 | 0.56 | 0.20 | 3.05 | 3.02 |
| shared-prime | 100 | 32 | 2.31 | 0.88 | 9.91 | 7.97 |
| shared-prime | 100 | 1024 | 43.01 | 13.40 | 165.76 | 141.77 |
| shared-prime | 128 | 1 | 0.23 | 0.09 | 1.15 | 1.25 |
| shared-prime | 128 | 3 | 0.53 | 0.20 | 2.85 | 3.01 |
| shared-prime | 128 | 32 | 2.42 | 0.87 | 9.75 | 8.12 |
| shared-prime | 128 | 1024 | 42.95 | 13.17 | 169.75 | 135.84 |

Candidate [joint-commit-timing] encoding includes allocation and replicate fill; baseline [commit-timing] ntt excludes them. Only Merkle timers and comparable enclosing phases are directly compared.

Nested tracing spans overlap. The JSON artifact retains each named span total for inspection; summing all spans would double-count time.
