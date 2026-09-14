# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_ood | log10 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.146 / 3.453 | 0.016 | 13 / 32832 | pass |
| aarch64 | opt_ood | log10 | reuse_products | yes | 0.715 [0.710, 0.724] | 0.731 [0.707, 0.756] | 2.257 / 2.543 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | indexed_products |  | 0.752 [0.745, 0.758] | 0.754 [0.728, 0.768] | 2.360 / 2.590 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | collect_products |  | 0.762 [0.752, 0.766] | 0.769 [0.742, 0.793] | 2.384 / 2.596 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | vector_products |  | 0.760 [0.751, 0.770] | 0.769 [0.737, 0.779] | 2.387 / 2.642 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | prepared_products |  | 0.744 [0.740, 0.756] | 0.755 [0.738, 0.790] | 2.340 / 2.638 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log16 | production_blocked | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 92.956 / 114.641 | 1.000 | 20 / 133328 | pass |
| aarch64 | opt_ood | log16 | reuse_products |  | 1.138 [1.082, 1.178] | 1.152 [1.005, 1.324] | 105.076 / 128.876 | 1.000 | 3 / 67312 | regression |
| aarch64 | opt_ood | log16 | indexed_products |  | 1.149 [1.125, 1.207] | 1.154 [1.019, 1.366] | 108.586 / 142.977 | 1.000 | 3 / 67312 | regression |
| aarch64 | opt_ood | log16 | collect_products |  | 1.161 [1.113, 1.210] | 1.164 [1.043, 1.276] | 107.833 / 134.648 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log16 | vector_products |  | 1.149 [1.103, 1.197] | 1.111 [0.925, 1.292] | 107.166 / 133.611 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log16 | prepared_products |  | 1.142 [1.107, 1.196] | 1.160 [0.886, 1.238] | 107.567 / 132.371 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log18 | production_blocked | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 131.510 / 174.222 | 4.000 | 22 / 135632 | pass |
| aarch64 | opt_ood | log18 | reuse_products |  | 1.089 [1.061, 1.122] | 1.104 [0.921, 1.264] | 142.189 / 176.430 | 4.000 | 3 / 68080 | regression |
| aarch64 | opt_ood | log18 | indexed_products |  | 1.105 [1.070, 1.133] | 0.997 [0.682, 1.108] | 144.286 / 181.694 | 4.000 | 3 / 68080 | regression |
| aarch64 | opt_ood | log18 | collect_products |  | 1.105 [1.078, 1.136] | 1.109 [0.875, 1.198] | 144.232 / 191.274 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood | log18 | vector_products |  | 1.118 [1.086, 1.170] | 1.045 [0.754, 1.162] | 146.717 / 193.396 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood | log18 | prepared_products |  | 1.127 [1.089, 1.152] | 1.019 [0.621, 1.152] | 146.731 / 180.689 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood_reuse | log10 | allocate |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.261 / 2.645 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood_reuse | log10 | scratch | yes | 0.828 [0.819, 0.840] | 0.826 [0.800, 0.844] | 1.886 / 2.276 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_ood_reuse | log10 | indexed_scratch |  | 0.860 [0.845, 0.866] | 0.851 [0.717, 0.868] | 1.954 / 2.170 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_ood_reuse | log16 | allocate | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 106.047 / 117.088 | 1.000 | 3 / 67312 | pass |
| aarch64 | opt_ood_reuse | log16 | scratch |  | 1.023 [0.979, 1.065] | 1.002 [0.961, 1.079] | 107.592 / 121.299 | 1.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ood_reuse | log16 | indexed_scratch |  | 1.043 [1.020, 1.097] | 1.022 [0.989, 1.101] | 109.574 / 122.806 | 1.000 | 1 / 1520 | regression |
| aarch64 | opt_ood_reuse | log18 | allocate | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 144.880 / 203.014 | 4.000 | 3 / 68080 | pass |
| aarch64 | opt_ood_reuse | log18 | scratch |  | 1.021 [0.997, 1.057] | 0.992 [0.961, 1.270] | 147.500 / 202.633 | 4.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ood_reuse | log18 | indexed_scratch |  | 1.037 [1.001, 1.059] | 1.039 [0.958, 1.113] | 150.829 / 192.564 | 4.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log8_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 46.805 / 49.189 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | depth_first |  | 0.865 [0.860, 0.871] | 0.869 [0.858, 0.885] | 40.540 / 42.602 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | tiled_half |  | 0.663 [0.658, 0.667] | 0.668 [0.658, 0.734] | 31.048 / 33.002 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | half_depth | yes | 0.666 [0.651, 0.670] | 0.664 [0.650, 0.677] | 31.125 / 32.596 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 475.320 / 540.291 | 1.500 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log12_lanes8 | depth_first |  | 0.531 [0.493, 0.551] | 0.504 [0.473, 0.529] | 247.292 / 272.776 | 1.500 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log12_lanes8 | tiled_half |  | 0.594 [0.560, 0.615] | 0.601 [0.529, 0.621] | 276.852 / 323.342 | 1.500 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log12_lanes8 | half_depth | yes | 0.503 [0.465, 0.517] | 0.465 [0.434, 0.497] | 231.651 / 251.204 | 1.500 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1320.656 / 1916.056 | 12.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes8 | depth_first |  | 0.723 [0.652, 0.739] | 0.775 [0.625, 0.847] | 940.078 / 1438.874 | 12.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes8 | tiled_half |  | 0.905 [0.878, 0.937] | 0.964 [0.809, 1.173] | 1180.739 / 2018.925 | 12.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log15_lanes8 | half_depth | yes | 0.668 [0.637, 0.712] | 0.689 [0.574, 0.777] | 894.516 / 1373.073 | 12.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2996.010 / 7611.852 | 48.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes32 | depth_first |  | 0.932 [0.766, 0.980] | 1.006 [0.760, 1.394] | 2834.354 / 5488.746 | 48.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log15_lanes32 | tiled_half |  | 0.988 [0.936, 1.021] | 1.012 [0.861, 1.134] | 2923.459 / 7355.171 | 48.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log15_lanes32 | half_depth | yes | 0.899 [0.767, 0.944] | 0.924 [0.794, 1.172] | 2690.729 / 6102.856 | 48.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6099.520 / 9164.685 | 96.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log16_lanes32 | depth_first |  | 0.947 [0.894, 1.002] | 0.937 [0.674, 1.098] | 5887.750 / 7820.042 | 96.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | tiled_half |  | 1.022 [0.968, 1.073] | 1.021 [0.751, 1.204] | 6253.645 / 8715.617 | 96.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | half_depth | yes | 0.891 [0.845, 0.961] | 1.128 [0.664, 1.344] | 5607.583 / 8931.433 | 96.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24797.355 / 44876.132 | 384.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log18_lanes32 | depth_first |  | 0.932 [0.898, 0.996] | 1.050 [0.727, 2.277] | 23696.645 / 36193.009 | 384.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | tiled_half |  | 0.962 [0.919, 1.003] | 0.952 [0.744, 1.268] | 23850.520 / 45195.309 | 384.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | half_depth | yes | 0.904 [0.851, 0.954] | 0.920 [0.692, 1.173] | 22781.229 / 36177.687 | 384.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log8_lanes1 | production | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.705 / 3.213 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | 1.343 [1.305, 1.377] | 1.290 [1.213, 1.387] | 3.619 / 4.217 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | tiled_half |  | 1.118 [1.091, 1.148] | 1.085 [0.998, 1.149] | 3.012 / 3.491 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | half_depth |  | 1.120 [1.094, 1.143] | 1.042 [0.993, 1.156] | 3.015 / 3.474 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_pack | logs9_9 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 31.063 / 39.419 | 0.016 | 2 / 17904 | pass |
| aarch64 | opt_pack | logs9_9 | single_write |  | 0.024 [0.020, 0.027] | 0.024 [0.019, 0.027] | 0.733 / 0.867 | 0.016 | 1 / 16384 | pass |
| aarch64 | opt_pack | logs9_9 | tiled_write | yes | 0.020 [0.016, 0.023] | 0.018 [0.016, 0.021] | 0.609 / 0.714 | 0.016 | 1 / 16384 | pass |
| aarch64 | opt_pack | logs16_14 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 210.557 / 858.080 | 2.000 | 2 / 2098672 | pass |
| aarch64 | opt_pack | logs16_14 | single_write |  | 0.469 [0.355, 0.489] | 0.409 [0.139, 0.461] | 96.521 / 210.270 | 2.000 | 1 / 2097152 | pass |
| aarch64 | opt_pack | logs16_14 | tiled_write | yes | 0.429 [0.412, 0.606] | 0.500 [0.380, 0.943] | 89.349 / 568.668 | 2.000 | 2 / 2098672 | pass |
| aarch64 | opt_pack | logs18_18 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 472.398 / 855.096 | 8.000 | 2 / 8390128 | pass |
| aarch64 | opt_pack | logs18_18 | single_write |  | 0.982 [0.826, 1.044] | 0.786 [0.545, 0.960] | 453.703 / 560.832 | 8.000 | 1 / 8388608 | inconclusive |
| aarch64 | opt_pack | logs18_18 | tiled_write | yes | 0.385 [0.371, 0.431] | 0.420 [0.330, 0.715] | 190.156 / 394.227 | 8.000 | 2 / 8390128 | pass |
| aarch64 | opt_packed_ood | logs9_9 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 52.085 / 70.521 | 0.031 | 15 / 50736 | pass |
| aarch64 | opt_packed_ood | logs9_9 | single_write_ood |  | 0.065 [0.047, 0.072] | 0.062 [0.044, 0.075] | 3.069 / 4.744 | 0.031 | 3 / 32784 | pass |
| aarch64 | opt_packed_ood | logs9_9 | tiled_indexed_ood | yes | 0.065 [0.047, 0.074] | 0.068 [0.044, 0.140] | 3.030 / 6.160 | 0.031 | 3 / 32784 | pass |
| aarch64 | opt_packed_ood | logs16_14 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 335.367 / 796.009 | 4.000 | 22 / 2231248 | pass |
| aarch64 | opt_packed_ood | logs16_14 | single_write_ood |  | 0.810 [0.757, 0.852] | 0.801 [0.696, 0.919] | 269.966 / 769.859 | 4.000 | 4 / 2164720 | pass |
| aarch64 | opt_packed_ood | logs16_14 | tiled_indexed_ood | yes | 0.584 [0.547, 0.748] | 0.598 [0.571, 1.383] | 192.111 / 1596.290 | 4.000 | 4 / 2164720 | inconclusive |
| aarch64 | opt_packed_ood | logs18_18 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 673.099 / 1550.406 | 16.000 | 24 / 8527312 | pass |
| aarch64 | opt_packed_ood | logs18_18 | single_write_ood |  | 1.085 [0.895, 1.180] | 1.083 [0.812, 1.259] | 726.398 / 1411.868 | 16.000 | 4 / 8457712 | inconclusive |
| aarch64 | opt_packed_ood | logs18_18 | tiled_indexed_ood | yes | 0.575 [0.545, 0.652] | 0.592 [0.458, 0.772] | 388.698 / 1198.994 | 16.000 | 4 / 8457712 | pass |
