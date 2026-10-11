# Binding strategy selection evidence

Two-seed pilot only; negative deltas mean faster relative to the same binary’s actual default. All prior failures remain relevant until a production binary passes full qualification.

## N512, eight workers

| Batch | Security | opt3 total / proof Δ | opt4 total / proof Δ | opt4 vs opt3 total / proof Δ |
|---:|---:|---:|---:|---:|
| 17 | 100 | -6.07% / -6.03% | -5.68% / -5.45% | +0.42% / +0.61% |
| 17 | 128 | -5.02% / -5.38% | -5.38% / -5.50% | -0.38% / -0.13% |
| 32 | 100 | -8.28% / -7.92% | -9.60% / -7.92% | -1.44% / +0.00% |
| 32 | 128 | -5.84% / -4.68% | -5.77% / -5.96% | +0.08% / -1.34% |
| 33 | 100 | -8.46% / -8.72% | -11.04% / -11.04% | -2.82% / -2.55% |
| 33 | 128 | -6.10% / -6.81% | -7.08% / -7.73% | -1.05% / -0.98% |
| 65 | 100 | -13.54% / -14.86% | -16.33% / -17.90% | -3.23% / -3.57% |
| 65 | 128 | -9.96% / -10.82% | -11.75% / -12.50% | -2.00% / -1.88% |
| 129 | 100 | -15.30% / -17.22% | -18.71% / -20.92% | -4.02% / -4.47% |
| 129 | 128 | -13.89% / -14.83% | -15.98% / -17.36% | -2.42% / -2.98% |
| 1024 | 100 | -0.25% / -0.48% | -1.28% / -1.28% | -1.03% / -0.81% |
| 1024 | 128 | -0.31% / -0.76% | -1.91% / -2.03% | -1.60% / -1.28% |

## N512, one worker

| Batch | Security | opt3 total / proof Δ | opt4 total / proof Δ | opt4 vs opt3 total / proof Δ |
|---:|---:|---:|---:|---:|
| 16 | 100 | -1.20% / -1.18% | -1.32% / -1.33% | -0.13% / -0.16% |
| 16 | 128 | -1.18% / -1.23% | -1.14% / -1.19% | +0.05% / +0.04% |
| 17 | 100 | -1.61% / -1.64% | -3.99% / -4.21% | -2.42% / -2.61% |
| 17 | 128 | -1.47% / -1.52% | -4.21% / -4.33% | -2.78% / -2.86% |
| 32 | 100 | -1.35% / -1.44% | -2.22% / -2.43% | -0.88% / -1.00% |
| 32 | 128 | -0.89% / -0.97% | -1.59% / -1.68% | -0.70% / -0.72% |
| 33 | 100 | -1.61% / -1.70% | -6.02% / -6.34% | -4.48% / -4.72% |
| 33 | 128 | -2.21% / -2.32% | -5.37% / -5.65% | -3.23% / -3.41% |
| 65 | 100 | -2.26% / -2.44% | -7.70% / -8.28% | -5.57% / -5.99% |
| 65 | 128 | -1.60% / -1.70% | -7.20% / -7.64% | -5.69% / -6.04% |
| 129 | 100 | -2.26% / -2.46% | -8.32% / -9.05% | -6.20% / -6.76% |
| 129 | 128 | -2.01% / -2.16% | -7.79% / -8.42% | -5.90% / -6.40% |
| 1024 | 100 | -0.78% / -0.87% | +0.21% / +0.28% | +1.00% / +1.16% |
| 1024 | 128 | -1.04% / -1.21% | -0.61% / -0.69% | +0.44% / +0.52% |

## Conservative public-threshold candidate

- Preserve the existing strategy below the old half-floor: N512 batch≤16 and N1024 batch≤8. The padding improvement is already strong there; small strategy differences lack robust pilot support.
- For measured worker counts 1 and 8, use optimized binding above that threshold. Leave other worker counts on their existing policy until measured.
- One worker: prefix four above the half-floor, retaining the existing batch1024 prefix-four path.
- N1024 with eight workers: prefix four above the half-floor. The batch1024 improvement is about6.7–7.2% proof in both security targets.
- N512 with eight workers: prefix three through batch32; prefix four for batches33–255; preserve the existing qualified prefix-three strategy for batches256–1024. This adds the clearly winning middle range while avoiding a large-range switch based on only modest gains.

The N512 eight-worker batch32 preference varies between the initial pilot and broad sweep, so prefix three is the conservative threshold choice. The large N512 alternatives offer modest gains relative to the no-op control scatter, whereas middle ranges show much larger reductions. This is a proposed implementation policy, not a qualified performance claim.

## No-op control caveat

Each shape has a mode that uses exactly the actual default strategy: legacy3 except at batch1024, where one worker uses opt4 and eight workers use opt3. These controls are needed to calibrate noise. The first N512/security100/batch1/one-worker shape has an anomalous −10.65% proof no-op difference; it must not be interpreted as a computation gain. Other no-op observations are much smaller, but eight-worker total timings show visible noise. The pilot does not justify changing the batch1 strategy.
