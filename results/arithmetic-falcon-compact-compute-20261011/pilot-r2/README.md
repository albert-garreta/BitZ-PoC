# Revised v8 diagnostic pilot (R2)

The combined implementation **does not qualify**. Arithmetic-padding reduction retains substantial batch-1 gains, and revised compact bridges improve medium single-thread cases at security 100. Medium eight-thread cases still regress, and security 128 adds substantial compact costs in both thread settings. The fixed wider qualification has not started.

All **768 proofs verified**, with matching input digests across all four modes. All **192 exact mode no-op comparisons** and **240 exact v7 proof comparisons** passed. The v7 audit covers every candidate proof that reports the preserved `/v7` protocol; new `/v8` proofs are recorded separately. These checks include the full proof Debug digest, root, payload size, physical source size, input, protocol, trial and iteration. See [candidate provenance](../candidate-r2-provenance.json).

The predeclared diagnostic matrix has 32 shapes at security 100/128, seeds 42/43, one warmup and two measured proofs per mode/input. Modes run sequentially in forward/reverse order. No builds, tests or other benchmarks ran concurrently. Each cell below is total proving Δ / proof-only Δ relative to the qualified v7 control, using four measured values per mode/shape. Positive means slower.

| N | Security | Batch | Threads | Padding Δ total / proof | Compact Δ total / proof | Combined Δ total / proof |
|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | 1 | -31.75% / -31.96% | -7.95% / -7.89% | -31.68% / -31.89% |
| 512 | 100 | 1 | 8 | -20.15% / -20.79% | -0.34% / -0.26% | -20.61% / -21.24% |
| 512 | 100 | 32 | 1 | +0.53% / +0.58% | +0.68% / +0.69% | +0.77% / +0.81% |
| 512 | 100 | 32 | 8 | +0.80% / +1.90% | +1.18% / +2.74% | +0.95% / +1.29% |
| 512 | 100 | 1024 | 1 | +0.14% / +0.41% | +1.70% / +2.28% | +1.78% / +2.37% |
| 512 | 100 | 1024 | 8 | -1.04% / -0.92% | +0.17% / +0.43% | -0.50% / +0.18% |
| 512 | 100 | 33 | 1 | +0.48% / +0.53% | -4.37% / -4.59% | -4.55% / -4.77% |
| 512 | 100 | 33 | 8 | +0.69% / +0.07% | +12.96% / +14.05% | +13.03% / +13.88% |
| 1024 | 100 | 1 | 1 | -24.41% / -24.77% | -0.22% / -0.22% | -24.53% / -24.89% |
| 1024 | 100 | 1 | 8 | -17.21% / -18.03% | -0.46% / -0.50% | -17.13% / -18.40% |
| 1024 | 100 | 32 | 1 | -0.21% / -0.34% | -4.78% / -5.56% | -4.93% / -5.70% |
| 1024 | 100 | 32 | 8 | +0.52% / +0.60% | +10.92% / +10.89% | +10.09% / +10.57% |
| 1024 | 100 | 1024 | 1 | -0.51% / +0.05% | -0.38% / +0.27% | -0.62% / +0.01% |
| 1024 | 100 | 1024 | 8 | +0.08% / +0.42% | -3.01% / -2.93% | -3.20% / -2.87% |
| 1024 | 100 | 17 | 1 | +0.11% / +0.09% | -5.56% / -6.01% | -5.43% / -5.88% |
| 1024 | 100 | 17 | 8 | +0.53% / +0.71% | +12.44% / +13.65% | +12.07% / +13.28% |
| 512 | 128 | 1 | 1 | -28.45% / -28.70% | -0.06% / -0.07% | -28.66% / -28.92% |
| 512 | 128 | 1 | 8 | -26.75% / -27.33% | -1.06% / -1.12% | -27.02% / -27.61% |
| 512 | 128 | 32 | 1 | +0.22% / -0.18% | +0.02% / +0.01% | -0.04% / -0.04% |
| 512 | 128 | 32 | 8 | -0.94% / -1.28% | -0.23% / -0.24% | -0.90% / -0.95% |
| 512 | 128 | 1024 | 1 | -0.85% / -0.74% | +6.33% / +7.56% | +5.98% / +7.16% |
| 512 | 128 | 1024 | 8 | +0.99% / +0.69% | +0.37% / +0.86% | +0.11% / +0.74% |
| 512 | 128 | 33 | 1 | +0.13% / +0.25% | +23.24% / +24.67% | +23.37% / +24.79% |
| 512 | 128 | 33 | 8 | -0.07% / -0.08% | +36.45% / +38.95% | +37.43% / +40.35% |
| 1024 | 128 | 1 | 1 | -26.15% / -26.47% | +0.24% / +0.24% | -26.21% / -26.59% |
| 1024 | 128 | 1 | 8 | -24.69% / -25.45% | +0.50% / +0.55% | -24.40% / -25.15% |
| 1024 | 128 | 32 | 1 | +0.83% / +0.83% | +19.14% / +21.30% | +19.04% / +21.24% |
| 1024 | 128 | 32 | 8 | +0.14% / -0.64% | +26.18% / +29.04% | +27.46% / +29.44% |
| 1024 | 128 | 1024 | 1 | -0.41% / -0.19% | +2.44% / +3.48% | +2.46% / +3.47% |
| 1024 | 128 | 1024 | 8 | -0.95% / -0.89% | -3.22% / -2.85% | -3.16% / -3.09% |
| 1024 | 128 | 17 | 1 | +0.94% / +0.94% | +21.84% / +23.45% | +21.23% / +22.82% |
| 1024 | 128 | 17 | 8 | -0.66% / -0.66% | +33.28% / +35.70% | +32.63% / +35.11% |

Observed zero-threshold gates (either metric positive):

- `padding`: 29 positive metric entries in 16/32 shapes; observed gate fails.
- `compact`: 39 positive metric entries in 20/32 shapes; observed gate fails.
- `combined`: 32 positive metric entries in 17/32 shapes; observed gate fails.

These pooled-median gates preserve every positive observation; they do not establish that a sub-percent difference is repeatable. The unchanged raw cases reproduce the original v7 proof exactly, which removes transcript-induced prime/grinding differences from those cases. Their remaining small timing differences need wider measurements. The large compact losses at medium sizes and security 128 already warrant another implementation decision.

The separate [R2 large-component profiles](../pilot-r2-profiles/README.md) show that compact binding remains more expensive, while security-128 BitZ savings are smaller or absent in these seed-42 diagnostics. Trace timings do not replace the untraced acceptance measurements.

`samples.json`, per-mode raw outputs, `summary.json`, `seed-summary.json`, `comparisons/`, both identity reports and `metadata.json` preserve the full evidence. The initial pilot and profile outputs remain unchanged.
