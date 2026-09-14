# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_ood | log10 | production_blocked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | reuse_products | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | indexed_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | collect_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | vector_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | prepared_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | production_blocked | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | reuse_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | indexed_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | collect_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | vector_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | prepared_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | production_blocked | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | reuse_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | indexed_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | collect_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | vector_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | prepared_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log10 | allocate |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log10 | scratch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log10 | indexed_scratch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log16 | allocate | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log16 | scratch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log16 | indexed_scratch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log18 | allocate | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log18 | scratch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log18 | indexed_scratch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | half_depth | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | half_depth | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | half_depth | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2897.687 / 4176.649 | 48.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes32 | depth_first |  | 0.938 [0.862, 0.972] | 0.983 [0.854, 1.065] | 2722.375 / 3803.904 | 48.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log15_lanes32 | tiled_half |  | 1.002 [0.973, 1.025] | 1.088 [0.953, 1.187] | 2903.552 / 4459.258 | 48.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log15_lanes32 | half_depth | yes | 0.904 [0.838, 0.941] | 0.915 [0.787, 1.043] | 2633.302 / 3576.600 | 48.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 5803.479 / 9145.681 | 96.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log16_lanes32 | depth_first |  | 0.955 [0.845, 1.002] | 0.995 [0.733, 1.130] | 5484.751 / 7832.055 | 96.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | tiled_half |  | 1.014 [0.974, 1.052] | 1.017 [0.957, 1.184] | 5904.208 / 9331.550 | 96.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | half_depth | yes | 0.885 [0.802, 0.954] | 1.024 [0.695, 1.115] | 5167.562 / 7485.427 | 96.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 22581.500 / 28607.378 | 384.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log18_lanes32 | depth_first |  | 0.977 [0.961, 0.998] | 0.913 [0.800, 1.042] | 22133.105 / 26285.562 | 384.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | tiled_half |  | 0.981 [0.961, 1.001] | 0.935 [0.823, 1.089] | 22264.292 / 26758.640 | 384.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | half_depth | yes | 0.939 [0.925, 0.968] | 0.920 [0.797, 0.975] | 21496.562 / 25304.089 | 384.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log8_lanes1 | production | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | half_depth |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs9_9 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs9_9 | single_write |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs9_9 | tiled_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs16_14 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs16_14 | single_write |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs16_14 | tiled_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs18_18 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs18_18 | single_write |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs18_18 | tiled_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs9_9 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs9_9 | single_write_ood |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs9_9 | tiled_indexed_ood | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs16_14 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 301.658 / 879.441 | 4.000 | 22 / 2231248 | pass |
| aarch64 | opt_packed_ood | logs16_14 | single_write_ood |  | 0.838 [0.557, 0.973] | 0.833 [0.590, 0.869] | 254.370 / 395.585 | 4.000 | 4 / 2164720 | pass |
| aarch64 | opt_packed_ood | logs16_14 | tiled_indexed_ood | yes | 0.584 [0.485, 0.637] | 0.602 [0.552, 0.649] | 173.954 / 388.579 | 4.000 | 4 / 2164720 | pass |
| aarch64 | opt_packed_ood | logs18_18 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs18_18 | single_write_ood |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs18_18 | tiled_indexed_ood | yes | — | — | — | — | — | unmeasured |
