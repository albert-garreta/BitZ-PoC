# V5/V6 retained-data comparison

The same seeds do not reproduce the prover slowdowns. Across all 60 Native/128/batch-1 seed pairs, V5/V6 prover-delta correlation is −0.303 (−0.182 after centering each position in the balanced four-position schedule). The identical baseline binary has essentially zero per-seed prover correlation between campaigns (−0.018). The ten largest positive paired deltas have **no seed overlap**, and 35 of 60 deltas change sign.

Every corresponding baseline and candidate proof across the two campaigns matches exactly in Debug digest, input digest, source root, and grinding record (120 variant/seed comparisons). Input or nonce changes do not explain the difference. [Full data, correlations, and script outputs](v5-comparison.json), [analysis script](../compare_failed_v5_v6.py).

A consistent measured pattern is speedup across the three samples after the prescribed single warmup. The figures below include every retained trial and no exclusions.

| Campaign / side | Sample 1 prover, ms | Sample 2 prover, ms | Sample 3 prover, ms | Commit samples 1 → 2 → 3, ms |
|---|---:|---:|---:|---|
| v5 / baseline | 36.920 | 32.714 | 30.856 | 0.8733 → 0.7754 → 0.6837 |
| v5 / candidate | 36.386 | 33.863 | 30.383 | 0.8370 → 0.8098 → 0.6785 |
| v6 / baseline | 36.492 | 32.537 | 31.232 | 0.8190 → 0.7498 → 0.6865 |
| v6 / candidate | 36.676 | 33.850 | 30.240 | 0.8422 → 0.7927 → 0.6756 |

| Campaign / side | Prover median is sample 2 | Strictly decreasing sample times | Samples >10% above own process median | Those also >10% in commitment |
|---|---:|---:|---:|---:|
| v5 / baseline | 48/60 | 48/60 | 40/180 | 16 |
| v5 / candidate | 55/60 | 54/60 | 22/180 | 6 |
| v6 / baseline | 45/60 | 42/60 | 34/180 | 17 |
| v6 / candidate | 57/60 | 57/60 | 20/180 | 12 |

The 10% threshold is a descriptive marker, not an exclusion criterion. Baseline has more elevated trials than candidate under this definition. Whole-trial prover/commit correlations are 0.499/0.780 for V5 baseline/candidate and 0.656/0.831 for V6. This supports an execution-state component shared by commitment and proving; it does not identify CPU power policy, caches, scheduling, or another cause.

The accepted estimator remains the arithmetic mean of each seed’s three-sample median. For sensitivity analysis only, averaging all measured trials gives different point estimates:

| Campaign | Accepted mean-of-medians ratio | Descriptive all-trial mean ratio |
|---|---:|---:|
| v5 | 1.030148 | 1.001422 |
| v6 | 1.031210 | 1.005048 |

**These alternative descriptive numbers do not change either failure or qualify a build.** The campaigns remain not qualified under the agreed method. The data show that the selected middle-of-three statistic interacts with a strong within-process trend; they do not establish whether that trend is a benchmark condition, an implementation behavior, or both.

Both order groups and all schedule positions are retained in the JSON. Schedule-position differences also change between campaigns: V5’s largest excess was position 2 (candidate first), whereas V6 position 2 favors candidate. This weakens an explanation tied to one persistent seed or one process-order position. No new benchmark was needed for this comparison.
