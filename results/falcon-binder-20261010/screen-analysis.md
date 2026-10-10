# Falcon binder screening: first candidate

All 240 processes completed and verified: Falcon-512, batch 1024, security 128; seeds 42/43/44; 1/2/4/8/16 workers. Each process had one warmup and three measured trials. Values below compare process medians, paired by seed. These are screening measurements; fresh holdout runs are required.

## Selected candidates versus commit 51b472c55

| Workers | Arithmetic all kernels: total reduction | Arithmetic word: total reduction | Arithmetic word: prove reduction | Word RSS change | Full replay: total reduction |
|---:|---:|---:|---:|---:|---:|
| 1 | -2.67% | +25.82% | +32.59% | +0.60% | -0.40% |
| 2 | +10.51% | +31.54% | +37.59% | -0.02% | +7.65% |
| 4 | +17.00% | +33.17% | +38.16% | -0.38% | +5.24% |
| 8 | +0.55% | +12.68% | +15.81% | +13.75% | +5.30% |
| 16 | +25.00% | +26.55% | +29.05% | -0.95% | +3.02% |

A negative reduction is a slowdown. RSS is the process high-water mark, including setup and proof-digest formatting. The bit protocol preserves identical complete proof contents. The word protocol has a distinct domain and proof; its input and source commitments still match.

The latency shortlist is the word binder with optimized construction at every worker count, and optimized full replay at 2/4/8/16 workers. Full replay is already enabled in the one-worker baseline, making that case a regression control. Keep arithmetic `r111` in holdout as the cheaper-memory alternative, especially at 16 workers, where the word binder gains only about 2.1% over `r111` while using about 13% more RSS. The first word implementation does not meet memory nonregression at eight workers: RSS grows about 13.75%. The new memory-reuse implementation must be remeasured.

## Factorial effects within the candidate bit protocol

Each entry is the average percentage reduction in total prover time from enabling that flag, with the other two flags held fixed, averaged geometrically over four flag combinations and three seeds. These twelve matched pairs are clustered within three seeds. Construction and evaluation include their existing parallel dispatch, so the figures measure the whole selectable kernel, not only one arithmetic trick.

| Workers | Folded replay | Factored construction | Batched evaluation |
|---:|---:|---:|---:|
| 1 | 2.50% | 1.53% | 1.53% |
| 2 | 3.66% | 5.75% | 2.96% |
| 4 | 3.24% | 10.72% | 5.69% |
| 8 | 2.39% | 16.31% | 8.38% |
| 16 | 1.67% | 17.12% | 8.14% |

Construction supplies the largest multithread gain, followed by endpoint evaluation and replay. At one worker all three were already enabled in the baseline. At eight workers construction and evaluation were already enabled, explaining the small incremental gain from `r111` there. Across the word-binder runs, enabling construction also wins at every worker count; leaving it off regresses total latency at eight workers.

## Commit-stage regression to investigate

The candidate commits more slowly even when it uses the original bit protocol and original kernel policy. This is consistent across samples rather than an isolated warmup or outlier. At one worker, seed 44, baseline commit samples were 34.131/34.158/34.341 ms; candidate `auto` samples were 42.150/42.246/42.281 ms. `r111` and word samples show the same extra cost.

| Workers | Median candidate auto minus baseline commit time |
|---:|---:|
| 1 | +7.975 ms |
| 2 | +3.664 ms |
| 4 | +1.618 ms |
| 8 | +1.109 ms |
| 16 | +0.366 ms |

The extra time approximately scales down with worker count. The bodies of `commit_from_s2`, witness derivation, packing and source commitment were not changed; the cause is unestablished. Separate stage traces and the planned holdout should determine which parallel commit substage changed. Do not attribute this observation to the binder kernels themselves.

## Holdout recommendation

- Rebuild and test the memory-reuse candidate, then use fresh seeds and independent process repetitions.
- Arithmetic: baseline, all three bit kernels, and word binder with optimized construction at 1/2/4/8/16 workers.
- Full: baseline and optimized replay at 1/2/4/8/16 workers; one worker should retain its already enabled policy.
- Evaluate total prover time, prove-only time, and RSS separately. Investigate the commit regression and require eight-worker word memory nonregression before adopting it under that requirement.
- Keep stage logging out of acceptance runs. Report inconclusive intervals as inconclusive, rather than as nonregression.

Exact binary hashes, all configuration comparisons, per-seed ratios and factorial effects are in `screen-analysis.json`. Raw verified outputs remain under `screen/runs/`.
