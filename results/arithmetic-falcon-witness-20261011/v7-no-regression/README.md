# V7 first no-regression qualification

The full 16-shape comparison verifies successfully, and 14 shapes improve both total prover and proof-only medians. The strict zero-regression timing gate remains **failed** for Falcon-512/security-100/batch-32 with one and eight threads. Their observed differences are small (+0.05% to +0.43%), so this two-seed run does not establish a sustained regression or a no-regression guarantee. More independent input coverage is needed before accepting those configurations.

Both variants use matched native release builds and original-Falcon inputs, pinned workers, one warmup and three measured proofs per seed (42,43). All 192 measured proofs and 64 warmups verified; every paired input digest matched. V7 changes the protocol/layout, so matching earlier proof roots/digests is intentionally not required. All previous evidence is preserved.

Positive Δ means slower. Times are milliseconds and are medians of six samples per variant/shape. Proof-only time excludes checked witness derivation and commitment; total prover time includes them.

| N | Security | Batch | Threads | Total before → after | Total Δ | Proof-only before → after | Proof Δ | Seed42 proof Δ | Seed43 proof Δ |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 32 | 1 | 17.5828 → 17.5916 | +0.05% | 16.4301 → 16.4463 | +0.10% | +0.90% | -0.05% |
| 512 | 100 | 32 | 8 | 5.5537 → 5.5625 | +0.16% | 5.2079 → 5.2303 | +0.43% | -1.78% | +0.93% |
| 512 | 100 | 1024 | 1 | 270.4836 → 265.6073 | -1.80% | 234.8036 → 229.0250 | -2.46% | -2.61% | -2.29% |
| 512 | 100 | 1024 | 8 | 55.7305 → 54.8756 | -1.53% | 49.9187 → 49.2874 | -1.26% | -1.59% | +1.87% |
| 512 | 128 | 32 | 1 | 21.1551 → 21.0744 | -0.38% | 19.9755 → 19.9268 | -0.24% | +0.94% | -0.31% |
| 512 | 128 | 32 | 8 | 8.0589 → 7.9193 | -1.73% | 7.7191 → 7.6181 | -1.31% | -1.26% | -1.09% |
| 512 | 128 | 1024 | 1 | 282.1521 → 274.6801 | -2.65% | 246.5163 → 237.9785 | -3.46% | -2.56% | -4.31% |
| 512 | 128 | 1024 | 8 | 61.3413 → 60.0376 | -2.13% | 55.5324 → 54.5699 | -1.73% | +1.12% | -14.39% |
| 1024 | 100 | 32 | 1 | 27.0898 → 26.6160 | -1.75% | 23.7337 → 23.3580 | -1.58% | -1.38% | -2.52% |
| 1024 | 100 | 32 | 8 | 8.1624 → 8.0008 | -1.98% | 7.1943 → 7.0901 | -1.45% | -2.25% | -0.02% |
| 1024 | 100 | 1024 | 1 | 504.8997 → 493.0776 | -2.34% | 402.2051 → 391.0313 | -2.78% | -2.62% | -2.78% |
| 1024 | 100 | 1024 | 8 | 108.4618 → 107.2270 | -1.14% | 93.3572 → 92.3844 | -1.04% | -1.16% | -0.63% |
| 1024 | 128 | 32 | 1 | 31.4531 → 30.1836 | -4.04% | 28.1147 → 26.9498 | -4.14% | -2.46% | -4.64% |
| 1024 | 128 | 32 | 8 | 11.0222 → 10.7574 | -2.40% | 10.1570 → 9.7804 | -3.71% | -0.34% | -7.36% |
| 1024 | 128 | 1024 | 1 | 524.5172 → 507.9046 | -3.17% | 421.1731 → 405.8616 | -3.64% | -4.00% | -2.80% |
| 1024 | 128 | 1024 | 8 | 114.8402 → 114.2690 | -0.50% | 99.7394 → 99.2775 | -0.46% | -1.12% | -0.32% |

Repeated trials of a single input repeat its deterministic Fiat–Shamir/grinding work. Independent seeds matter more than simply adding repetitions when comparing different protocol domains, particularly at security 128. Baseline/candidate order alternates across seeds to reduce ordering bias. The strict gate is an observed comparison, not a confidence interval; it records every positive median change and never lets commitment savings hide a slower proof-only median.

Artifacts: `summary.json` aggregates each shape; `seed-summary.json` separates independent inputs; `regression-gate.json` lists all failing metrics; `samples.json` and per-run logs retain every measured proof, warmup, command and verification result. `metadata.json` records binary hashes, build settings and machine details.
