# Initial v8 diagnostic pilot

The combined implementation does **not qualify** for the full performance run. Padding reduction improves batch-1 proving substantially, but compact computation adds large costs at medium Split sizes and does not consistently improve the large eight-thread cases. These are diagnostic measurements from two input seeds, not the wider fixed qualification. All failures remain preserved.

All **384 proofs verified**, inputs matched across all four variants, and **96 exact no-op proof comparisons passed**: 36 Single comparisons (`padding == combined`) and 60 Split comparisons (`compact == combined`). Proof roots, full Debug digests, payload sizes, protocol/input/trial identities and physical source sizes matched. The source and binary were frozen before the run; see the [candidate provenance](../candidate-pilot-provenance.json).

The accepted pilot uses security 100, seeds 42/43, one warmup and two measured proofs per mode/input. Variants run sequentially in forward/reverse order. No builds or tests ran concurrently. Positive deltas below mean slower than the qualified v7 control. Each cell is total proving Δ / proof-only Δ, using four measured timings per variant/shape.

| N | Batch | Threads | Padding Δ total / proof | Compact Δ total / proof | Combined Δ total / proof |
|---:|---:|---:|---:|---:|---:|
| 512 | 1 | 1 | -24.63% / -24.90% | +0.59% / +0.59% | -24.55% / -24.82% |
| 512 | 1 | 8 | -17.63% / -18.15% | +0.52% / +0.58% | -17.84% / -18.37% |
| 512 | 32 | 1 | +1.29% / +1.33% | +1.19% / +1.28% | +0.98% / +1.02% |
| 512 | 32 | 8 | +2.51% / +3.27% | +2.16% / +2.65% | +3.23% / +3.67% |
| 512 | 1024 | 1 | +0.21% / +0.76% | -5.33% / -5.65% | -5.40% / -5.72% |
| 512 | 1024 | 8 | -1.59% / -1.34% | +2.28% / +3.58% | +3.24% / +4.46% |
| 512 | 33 | 1 | +0.50% / +0.45% | +22.51% / +23.82% | +22.37% / +23.68% |
| 512 | 33 | 8 | -0.35% / -1.04% | +42.00% / +43.87% | +41.12% / +43.30% |
| 1024 | 1 | 1 | -24.04% / -24.45% | -0.15% / -0.17% | -24.05% / -24.47% |
| 1024 | 1 | 8 | -16.48% / -17.18% | +1.05% / +1.24% | -17.75% / -18.43% |
| 1024 | 32 | 1 | -0.29% / -0.36% | +18.90% / +21.50% | +18.40% / +20.95% |
| 1024 | 32 | 8 | -0.06% / +0.21% | +33.35% / +36.64% | +34.14% / +37.15% |
| 1024 | 1024 | 1 | -0.69% / -0.32% | -1.17% / -0.80% | -0.72% / -0.41% |
| 1024 | 1024 | 8 | +0.09% / -0.23% | +0.10% / +0.66% | +0.21% / +0.68% |
| 1024 | 17 | 1 | -0.35% / +0.05% | +19.97% / +22.20% | +19.88% / +22.11% |
| 1024 | 17 | 8 | +1.32% / +0.87% | +38.86% / +43.21% | +39.32% / +42.63% |

Observed gate entries per mode (either proving metric positive; every shape was retained):

- `padding`: 13 positive metric entries in 8/16 shapes; zero-threshold observed gate fails.
- `compact`: 26 positive metric entries in 13/16 shapes; zero-threshold observed gate fails.
- `combined`: 20 positive metric entries in 10/16 shapes; zero-threshold observed gate fails.

The tiny positive padding/no-op changes need wider seed coverage before being called repeatable regressions. The medium compact changes are large in both thread settings: combined proof-only time rises approximately 21–44%. The separate [component profiles](../pilot-profiles/README.md) locate most of the cost in additional BitZ bridge work, compact binding, and remaining setup/joint work. Trace measurements are kept separate from this untraced pilot.

Full qualification is on hold while those costs are addressed. No favorable subset from this pilot replaces the predeclared 64-shape plan. `comparisons/` contains complete per-mode pooled and per-seed summaries and both timing gates; `samples.json`, raw commands/logs, `metadata.json`, and `same-v8-proof-identity.json` retain the evidence.
