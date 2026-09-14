# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_ood | log10 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.103 / 3.338 | 0.016 | 13 / 32832 | pass |
| aarch64 | opt_ood | log10 | reuse_products |  | 0.719 [0.713, 0.730] | 0.720 [0.707, 0.736] | 2.234 / 2.403 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | indexed_products |  | 0.752 [0.746, 0.756] | 0.751 [0.742, 0.769] | 2.328 / 2.545 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | collect_products |  | 0.754 [0.748, 0.762] | 0.763 [0.750, 0.771] | 2.344 / 2.529 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | vector_products |  | 0.757 [0.752, 0.765] | 0.766 [0.746, 0.781] | 2.350 / 2.571 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | prepared_products |  | 0.745 [0.742, 0.752] | 0.749 [0.738, 0.763] | 2.319 / 2.492 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | tiled_1024 |  | 0.715 [0.706, 0.724] | 0.724 [0.704, 0.739] | 2.220 / 2.416 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_256 |  | 0.388 [0.382, 0.392] | 0.389 [0.384, 0.399] | 1.208 / 1.301 | 0.016 | 2 / 4160 | pass |
| aarch64 | opt_ood | log10 | vector_1024 |  | 0.750 [0.746, 0.754] | 0.748 [0.740, 0.762] | 2.325 / 2.523 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | vector_2048 |  | 0.741 [0.735, 0.753] | 0.750 [0.742, 0.764] | 2.319 / 2.524 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_collect |  | 0.721 [0.710, 0.730] | 0.725 [0.716, 0.741] | 2.236 / 2.427 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_grouped | yes | 0.713 [0.708, 0.723] | 0.721 [0.710, 0.737] | 2.220 / 2.420 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log16 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 91.751 / 102.461 | 1.000 | 20 / 133328 | pass |
| aarch64 | opt_ood | log16 | reuse_products |  | 1.185 [1.131, 1.208] | 1.157 [1.130, 1.843] | 107.318 / 117.708 | 1.000 | 3 / 67312 | regression |
| aarch64 | opt_ood | log16 | indexed_products |  | 1.168 [1.134, 1.207] | 1.175 [1.100, 1.203] | 107.439 / 118.862 | 1.000 | 3 / 67312 | regression |
| aarch64 | opt_ood | log16 | collect_products |  | 1.179 [1.157, 1.214] | 1.172 [1.122, 1.202] | 108.615 / 119.286 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log16 | vector_products |  | 1.188 [1.146, 1.216] | 1.149 [1.101, 1.214] | 108.234 / 119.458 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log16 | prepared_products |  | 1.176 [1.139, 1.201] | 1.186 [1.133, 1.240] | 107.217 / 118.665 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log16 | tiled_1024 |  | 0.519 [0.508, 0.538] | 0.498 [0.484, 0.523] | 47.327 / 51.629 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | tiled_256 |  | 0.519 [0.510, 0.540] | 0.501 [0.477, 0.516] | 47.612 / 51.443 | 1.000 | 2 / 8192 | pass |
| aarch64 | opt_ood | log16 | vector_1024 |  | 0.581 [0.569, 0.599] | 0.560 [0.540, 0.585] | 52.955 / 57.480 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | vector_2048 |  | 0.589 [0.577, 0.612] | 0.568 [0.547, 0.597] | 53.891 / 59.223 | 1.000 | 2 / 33280 | pass |
| aarch64 | opt_ood | log16 | tiled_collect |  | 0.519 [0.509, 0.532] | 0.500 [0.475, 0.521] | 47.273 / 51.730 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | tiled_grouped | yes | 0.518 [0.507, 0.534] | 0.493 [0.480, 0.521] | 47.280 / 51.450 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log18 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 125.667 / 142.826 | 4.000 | 22 / 135632 | pass |
| aarch64 | opt_ood | log18 | reuse_products |  | 1.132 [1.096, 1.167] | 1.123 [1.085, 1.141] | 141.658 / 158.560 | 4.000 | 3 / 68080 | regression |
| aarch64 | opt_ood | log18 | indexed_products |  | 1.157 [1.124, 1.189] | 1.138 [1.104, 1.182] | 143.764 / 164.342 | 4.000 | 3 / 68080 | regression |
| aarch64 | opt_ood | log18 | collect_products |  | 1.138 [1.107, 1.174] | 1.135 [1.100, 1.164] | 143.516 / 160.872 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood | log18 | vector_products |  | 1.150 [1.124, 1.185] | 1.167 [1.116, 1.194] | 144.361 / 165.067 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood | log18 | prepared_products |  | 1.135 [1.111, 1.181] | 1.154 [1.101, 1.184] | 142.878 / 161.875 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood | log18 | tiled_1024 |  | 1.031 [0.989, 1.058] | 1.026 [0.993, 1.055] | 126.886 / 148.715 | 4.000 | 3 / 22000 | inconclusive |
| aarch64 | opt_ood | log18 | tiled_256 |  | 1.081 [1.054, 1.139] | 1.142 [1.067, 1.206] | 135.476 / 169.351 | 4.000 | 3 / 22000 | regression |
| aarch64 | opt_ood | log18 | vector_1024 |  | 1.011 [0.986, 1.081] | 1.055 [1.012, 1.095] | 129.873 / 155.211 | 4.000 | 3 / 22000 | regression |
| aarch64 | opt_ood | log18 | vector_2048 |  | 1.088 [1.063, 1.140] | 1.113 [1.082, 1.164] | 138.739 / 160.565 | 4.000 | 3 / 36336 | regression |
| aarch64 | opt_ood | log18 | tiled_collect |  | 1.037 [0.991, 1.078] | 1.059 [1.009, 1.108] | 128.005 / 151.958 | 4.000 | 4 / 26096 | inconclusive |
| aarch64 | opt_ood | log18 | tiled_grouped | yes | 0.883 [0.859, 0.927] | 0.911 [0.874, 0.934] | 111.937 / 129.900 | 4.000 | 3 / 22000 | pass |
