# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_ood | log10 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.069 / 3.183 | 0.016 | 13 / 32832 | pass |
| aarch64 | opt_ood | log10 | reuse_products |  | 0.725 [0.718, 0.728] | 0.728 [0.718, 0.739] | 2.221 / 2.322 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | indexed_products |  | 0.751 [0.742, 0.761] | 0.759 [0.742, 0.767] | 2.308 / 2.409 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | collect_products |  | 0.760 [0.753, 0.764] | 0.767 [0.748, 0.782] | 2.330 / 2.451 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | vector_products |  | 0.761 [0.756, 0.767] | 0.769 [0.754, 0.788] | 2.337 / 2.449 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | prepared_products |  | 0.744 [0.741, 0.753] | 0.756 [0.738, 0.769] | 2.292 / 2.414 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | tiled_1024 |  | 0.718 [0.707, 0.722] | 0.719 [0.707, 0.731] | 2.195 / 2.297 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_256 |  | 0.390 [0.388, 0.394] | 0.397 [0.388, 0.413] | 1.198 / 1.273 | 0.016 | 2 / 4160 | pass |
| aarch64 | opt_ood | log10 | vector_1024 |  | 0.749 [0.744, 0.753] | 0.752 [0.731, 0.770] | 2.291 / 2.413 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | vector_2048 |  | 0.746 [0.743, 0.755] | 0.756 [0.739, 0.762] | 2.298 / 2.407 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_collect |  | 0.722 [0.718, 0.728] | 0.733 [0.716, 0.746] | 2.217 / 2.333 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_grouped | yes | 0.713 [0.709, 0.720] | 0.718 [0.706, 0.729] | 2.193 / 2.296 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log16 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 82.856 / 95.880 | 1.000 | 20 / 133328 | pass |
| aarch64 | opt_ood | log16 | reuse_products |  | 1.148 [1.123, 1.193] | 1.118 [1.079, 1.239] | 95.657 / 111.376 | 1.000 | 3 / 67312 | regression |
| aarch64 | opt_ood | log16 | indexed_products |  | 1.168 [1.128, 1.208] | 1.125 [1.079, 1.225] | 97.140 / 109.787 | 1.000 | 3 / 67312 | regression |
| aarch64 | opt_ood | log16 | collect_products |  | 1.165 [1.131, 1.214] | 1.133 [1.081, 1.183] | 97.530 / 110.392 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log16 | vector_products |  | 1.192 [1.156, 1.225] | 1.157 [1.110, 1.288] | 98.590 / 114.113 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log16 | prepared_products |  | 1.181 [1.144, 1.219] | 1.146 [1.091, 1.538] | 97.998 / 113.489 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log16 | tiled_1024 |  | 0.564 [0.534, 0.587] | 0.523 [0.476, 0.544] | 46.840 / 48.927 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | tiled_256 |  | 0.569 [0.536, 0.583] | 0.516 [0.473, 0.538] | 46.835 / 48.552 | 1.000 | 2 / 8192 | pass |
| aarch64 | opt_ood | log16 | vector_1024 |  | 0.629 [0.595, 0.655] | 0.571 [0.523, 0.605] | 52.204 / 54.035 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | vector_2048 |  | 0.647 [0.607, 0.666] | 0.594 [0.537, 0.630] | 53.390 / 55.706 | 1.000 | 2 / 33280 | pass |
| aarch64 | opt_ood | log16 | tiled_collect |  | 0.561 [0.534, 0.586] | 0.510 [0.473, 0.536] | 46.643 / 48.928 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | tiled_grouped | yes | 0.567 [0.531, 0.587] | 0.512 [0.480, 0.546] | 46.743 / 49.229 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log18 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 130.367 / 154.394 | 4.000 | 22 / 135632 | pass |
| aarch64 | opt_ood | log18 | reuse_products |  | 1.086 [1.067, 1.108] | 1.194 [1.018, 1.539] | 141.382 / 180.361 | 4.000 | 3 / 68080 | regression |
| aarch64 | opt_ood | log18 | indexed_products |  | 1.117 [1.085, 1.132] | 1.191 [1.062, 1.921] | 144.473 / 209.173 | 4.000 | 3 / 68080 | regression |
| aarch64 | opt_ood | log18 | collect_products |  | 1.091 [1.076, 1.116] | 1.103 [0.889, 1.444] | 143.105 / 166.880 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood | log18 | vector_products |  | 1.104 [1.091, 1.121] | 1.362 [1.069, 2.203] | 144.385 / 207.463 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood | log18 | prepared_products |  | 1.112 [1.090, 1.128] | 1.104 [0.964, 1.448] | 144.498 / 173.909 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood | log18 | tiled_1024 |  | 0.971 [0.958, 0.983] | 0.947 [0.819, 1.084] | 126.375 / 142.387 | 4.000 | 3 / 22000 | inconclusive |
| aarch64 | opt_ood | log18 | tiled_256 |  | 1.038 [1.012, 1.057] | 1.141 [0.922, 1.447] | 135.319 / 164.998 | 4.000 | 3 / 22000 | regression |
| aarch64 | opt_ood | log18 | vector_1024 |  | 1.004 [0.992, 1.019] | 1.087 [0.858, 1.477] | 130.836 / 159.997 | 4.000 | 3 / 22000 | inconclusive |
| aarch64 | opt_ood | log18 | vector_2048 |  | 1.100 [1.087, 1.127] | 1.108 [0.951, 1.316] | 143.189 / 171.646 | 4.000 | 3 / 36336 | regression |
| aarch64 | opt_ood | log18 | tiled_collect |  | 0.986 [0.974, 1.015] | 1.126 [0.908, 1.389] | 129.348 / 178.589 | 4.000 | 4 / 26096 | inconclusive |
| aarch64 | opt_ood | log18 | tiled_grouped | yes | 0.881 [0.860, 0.896] | 0.841 [0.649, 0.934] | 114.811 / 126.131 | 4.000 | 3 / 22000 | pass |
