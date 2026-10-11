# Broad same-binary binding strategy sweep

The optimized binding strategy plus a four-variable prefix gives substantial medium-batch improvements while producing identical proofs. This two-seed sweep covers all 64 qualification shapes to choose a computation policy; it is **not a production qualification**. The [policy evidence](POLICY.md) proposes conservative public batch/degree/worker thresholds and preserves existing small/large paths where gains are weak.

All **1,536 proofs verified**. All **1,152 strategy/default complete-proof comparisons** passed. Every proof also matched the earlier production qualification proof for the same input (including 1,152 proofs that transitively match original v7). Witness/commitment/payload/protocol changes are therefore absent across the strategy controls.

Every mode runs in the same native-release diagnostic binary with `falcon-hybrid,bench-internals` and `compute_variant=padding`. The `default` mode leaves both binding controls absent; legacy3/opt3/opt4 set them explicitly. This compares against the actual existing per-shape strategy: legacy3 normally, opt4 at batch1024 with one worker, and opt3 at batch1024 with eight workers. All inherited arithmetic/Falcon controls are cleared first.

The fixed sweep uses seeds42/43, one warmup and two measured proofs per mode/input, forward/reverse order, pinned one/eight workers, and sequential execution without concurrent builds/tests/profiles/analysis. Negative deltas below mean faster relative to actual default. The no-op column reports the explicitly selected strategy identical to default, measuring timing variation rather than a computation change.

| N | Security | Batch | Workers | opt3 total / proof Δ | opt4 total / proof Δ | Identical control | Control total / proof Δ |
|---:|---:|---:|---:|---:|---:|---|---:|
| 512 | 100 | 1 | 1 | -10.30% / -10.29% | -10.50% / -10.49% | legacy3 | -10.64% / -10.65% |
| 512 | 100 | 1 | 8 | -0.81% / -0.85% | +4.34% / +4.52% | legacy3 | +0.68% / -0.80% |
| 512 | 100 | 32 | 1 | -1.35% / -1.44% | -2.22% / -2.43% | legacy3 | -0.05% / -0.05% |
| 512 | 100 | 32 | 8 | -8.28% / -7.92% | -9.60% / -7.92% | legacy3 | -0.91% / +0.69% |
| 512 | 128 | 1 | 1 | -0.28% / -0.30% | -0.30% / -0.30% | legacy3 | -0.16% / -0.17% |
| 512 | 128 | 1 | 8 | -0.45% / -0.41% | -0.56% / -0.54% | legacy3 | +2.77% / -1.02% |
| 512 | 128 | 32 | 1 | -0.89% / -0.97% | -1.59% / -1.68% | legacy3 | +0.46% / +0.49% |
| 512 | 128 | 32 | 8 | -5.84% / -4.68% | -5.77% / -5.96% | legacy3 | -0.60% / -0.66% |
| 1024 | 100 | 1 | 1 | -0.07% / -0.08% | -0.40% / -0.40% | legacy3 | -0.14% / -0.16% |
| 1024 | 100 | 1 | 8 | +0.43% / +0.61% | +1.37% / +1.62% | legacy3 | +0.16% / -0.07% |
| 1024 | 100 | 32 | 1 | -2.33% / -2.59% | -6.51% / -7.34% | legacy3 | -0.29% / -0.26% |
| 1024 | 100 | 32 | 8 | -11.97% / -13.37% | -14.87% / -15.35% | legacy3 | -2.11% / +0.10% |
| 1024 | 128 | 1 | 1 | -0.09% / -0.12% | -0.43% / -0.44% | legacy3 | +0.13% / +0.13% |
| 1024 | 128 | 1 | 8 | -1.77% / -1.73% | -1.90% / -1.84% | legacy3 | -1.41% / -1.36% |
| 1024 | 128 | 32 | 1 | -2.01% / -2.24% | -5.69% / -6.35% | legacy3 | +0.04% / +0.03% |
| 1024 | 128 | 32 | 8 | -7.18% / -9.11% | -9.78% / -10.49% | legacy3 | +0.74% / -0.04% |
| 512 | 100 | 1024 | 1 | -0.78% / -0.87% | +0.21% / +0.28% | opt4 | +0.21% / +0.28% |
| 512 | 100 | 1024 | 8 | -0.25% / -0.48% | -1.28% / -1.28% | opt3 | -0.25% / -0.48% |
| 512 | 128 | 1024 | 1 | -1.04% / -1.21% | -0.61% / -0.69% | opt4 | -0.61% / -0.69% |
| 512 | 128 | 1024 | 8 | -0.31% / -0.76% | -1.91% / -2.03% | opt3 | -0.31% / -0.76% |
| 1024 | 100 | 1024 | 1 | +6.56% / +8.41% | -0.13% / +0.02% | opt4 | -0.13% / +0.02% |
| 1024 | 100 | 1024 | 8 | +0.59% / +0.35% | -5.54% / -6.69% | opt3 | +0.59% / +0.35% |
| 1024 | 128 | 1024 | 1 | +7.30% / +9.16% | +0.20% / +0.26% | opt4 | +0.20% / +0.26% |
| 1024 | 128 | 1024 | 8 | -0.67% / -0.50% | -6.27% / -7.17% | opt3 | -0.67% / -0.50% |
| 512 | 100 | 16 | 1 | -1.20% / -1.18% | -1.32% / -1.33% | legacy3 | +0.12% / +0.21% |
| 512 | 100 | 16 | 8 | -5.87% / -6.13% | -5.41% / -5.67% | legacy3 | -1.76% / -1.76% |
| 512 | 100 | 17 | 1 | -1.61% / -1.64% | -3.99% / -4.21% | legacy3 | -0.14% / -0.14% |
| 512 | 100 | 17 | 8 | -6.07% / -6.03% | -5.68% / -5.45% | legacy3 | +1.22% / +1.83% |
| 512 | 100 | 33 | 1 | -1.61% / -1.70% | -6.02% / -6.34% | legacy3 | +0.10% / +0.12% |
| 512 | 100 | 33 | 8 | -8.46% / -8.72% | -11.04% / -11.04% | legacy3 | +0.51% / +1.47% |
| 512 | 100 | 65 | 1 | -2.26% / -2.44% | -7.70% / -8.28% | legacy3 | -0.11% / -0.14% |
| 512 | 100 | 65 | 8 | -13.54% / -14.86% | -16.33% / -17.90% | legacy3 | -1.09% / -0.64% |
| 512 | 100 | 129 | 1 | -2.26% / -2.46% | -8.32% / -9.05% | legacy3 | -0.04% / -0.07% |
| 512 | 100 | 129 | 8 | -15.30% / -17.22% | -18.71% / -20.92% | legacy3 | +1.17% / +0.66% |
| 512 | 128 | 16 | 1 | -1.18% / -1.23% | -1.14% / -1.19% | legacy3 | -0.51% / -0.52% |
| 512 | 128 | 16 | 8 | -3.68% / -3.54% | -2.33% / -3.13% | legacy3 | +0.57% / +0.24% |
| 512 | 128 | 17 | 1 | -1.47% / -1.52% | -4.21% / -4.33% | legacy3 | -0.46% / -0.47% |
| 512 | 128 | 17 | 8 | -5.02% / -5.38% | -5.38% / -5.50% | legacy3 | -0.99% / -1.53% |
| 512 | 128 | 33 | 1 | -2.21% / -2.32% | -5.37% / -5.65% | legacy3 | -0.44% / -0.48% |
| 512 | 128 | 33 | 8 | -6.10% / -6.81% | -7.08% / -7.73% | legacy3 | +0.16% / +0.32% |
| 512 | 128 | 65 | 1 | -1.60% / -1.70% | -7.20% / -7.64% | legacy3 | -0.12% / -0.12% |
| 512 | 128 | 65 | 8 | -9.96% / -10.82% | -11.75% / -12.50% | legacy3 | -1.16% / -1.32% |
| 512 | 128 | 129 | 1 | -2.01% / -2.16% | -7.79% / -8.42% | legacy3 | +0.15% / +0.15% |
| 512 | 128 | 129 | 8 | -13.89% / -14.83% | -15.98% / -17.36% | legacy3 | -1.29% / -1.89% |
| 1024 | 100 | 8 | 1 | -1.49% / -1.57% | -3.35% / -3.54% | legacy3 | -0.17% / -0.17% |
| 1024 | 100 | 8 | 8 | -5.11% / -5.48% | -4.44% / -4.94% | legacy3 | +0.90% / +1.63% |
| 1024 | 100 | 9 | 1 | -1.30% / -1.39% | -5.97% / -6.34% | legacy3 | +0.16% / +0.13% |
| 1024 | 100 | 9 | 8 | -5.76% / -6.08% | -7.47% / -7.90% | legacy3 | +0.37% / +0.23% |
| 1024 | 100 | 17 | 1 | -1.23% / -1.33% | -8.14% / -8.81% | legacy3 | +0.31% / -0.02% |
| 1024 | 100 | 17 | 8 | -9.73% / -9.29% | -11.83% / -11.98% | legacy3 | +0.03% / +0.17% |
| 1024 | 100 | 65 | 1 | -1.84% / -2.03% | -12.25% / -13.74% | legacy3 | +1.02% / +1.17% |
| 1024 | 100 | 65 | 8 | -16.22% / -18.12% | -22.22% / -24.14% | legacy3 | -1.47% / -1.15% |
| 1024 | 100 | 129 | 1 | -3.22% / -3.67% | -13.53% / -15.41% | legacy3 | -0.22% / -0.28% |
| 1024 | 100 | 129 | 8 | -20.06% / -21.86% | -24.24% / -26.14% | legacy3 | -0.06% / -0.64% |
| 1024 | 128 | 8 | 1 | -0.95% / -1.00% | -2.54% / -2.66% | legacy3 | -0.11% / -0.09% |
| 1024 | 128 | 8 | 8 | -3.36% / -3.36% | -3.11% / -3.49% | legacy3 | -0.37% / -0.39% |
| 1024 | 128 | 9 | 1 | -0.92% / -0.95% | -5.07% / -5.34% | legacy3 | -0.02% / -0.02% |
| 1024 | 128 | 9 | 8 | -4.14% / -4.09% | -4.85% / -4.88% | legacy3 | +0.27% / +0.52% |
| 1024 | 128 | 17 | 1 | -0.82% / -0.82% | -7.17% / -7.68% | legacy3 | -0.04% / +0.01% |
| 1024 | 128 | 17 | 8 | -5.69% / -6.19% | -7.54% / -7.70% | legacy3 | +0.31% / +0.36% |
| 1024 | 128 | 65 | 1 | -2.07% / -2.25% | -11.54% / -12.80% | legacy3 | +0.51% / +0.62% |
| 1024 | 128 | 65 | 8 | -12.87% / -14.34% | -17.84% / -19.18% | legacy3 | +0.21% / -0.25% |
| 1024 | 128 | 129 | 1 | -2.51% / -2.80% | -12.31% / -13.87% | legacy3 | -0.28% / -0.27% |
| 1024 | 128 | 129 | 8 | -17.07% / -18.34% | -21.63% / -23.42% | legacy3 | +0.54% / +1.25% |

The first N512/security100/batch1/one-worker no-op control differs by −10.65% proof time despite identical strategy and proof. That anomaly remains in the evidence and cannot be treated as an algorithm gain. Other small differences and eight-worker total noise likewise require caution. Small-path strategy defaults should remain unchanged on this evidence. Broad independent-seed production qualification is still required after implementation.

See [metadata](metadata.json), [mode proof identity](v7-reference-proof-identity.json), [earlier proof identity](prior-proof-identity.json), [no-op observations](no-op-control-deltas.json), and [source/binary provenance](../binding-controls-provenance.json). Raw outputs, commands and per-seed timings remain available.
