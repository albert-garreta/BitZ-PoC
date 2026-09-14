# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_ood | log10 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.092 / 3.332 | 0.016 | 13 / 32832 | pass |
| aarch64 | opt_ood | log10 | reuse_products | yes | 0.722 [0.695, 0.729] | 0.731 [0.700, 0.754] | 2.238 / 2.354 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | indexed_products |  | 0.749 [0.726, 0.758] | 0.765 [0.739, 0.781] | 2.327 / 2.452 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | collect_products |  | 0.759 [0.730, 0.766] | 0.770 [0.747, 0.796] | 2.358 / 2.489 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | vector_products |  | 0.756 [0.734, 0.769] | 0.767 [0.742, 0.783] | 2.357 / 2.466 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | prepared_products |  | 0.750 [0.724, 0.762] | 0.754 [0.736, 0.776] | 2.329 / 2.445 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log16 | production_blocked | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 87.588 / 119.027 | 1.000 | 20 / 133328 | pass |
| aarch64 | opt_ood | log16 | reuse_products |  | 1.193 [1.131, 1.244] | 1.152 [0.641, 1.292] | 103.849 / 116.646 | 1.000 | 3 / 67312 | regression |
| aarch64 | opt_ood | log16 | indexed_products |  | 1.193 [1.128, 1.293] | 1.147 [0.631, 1.198] | 104.189 / 117.982 | 1.000 | 3 / 67312 | regression |
| aarch64 | opt_ood | log16 | collect_products |  | 1.185 [1.108, 1.263] | 1.128 [0.641, 1.686] | 102.361 / 121.176 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log16 | vector_products |  | 1.207 [1.150, 1.282] | 1.145 [0.816, 1.401] | 105.021 / 127.731 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log16 | prepared_products |  | 1.189 [1.133, 1.278] | 1.138 [0.855, 1.588] | 104.018 / 149.328 | 1.000 | 4 / 67568 | regression |
| aarch64 | opt_ood | log18 | production_blocked | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 129.783 / 172.410 | 4.000 | 22 / 135632 | pass |
| aarch64 | opt_ood | log18 | reuse_products |  | 1.079 [1.064, 1.182] | 1.097 [0.888, 1.413] | 141.539 / 203.115 | 4.000 | 3 / 68080 | regression |
| aarch64 | opt_ood | log18 | indexed_products |  | 1.136 [1.074, 1.183] | 1.074 [0.810, 1.225] | 144.262 / 195.617 | 4.000 | 3 / 68080 | regression |
| aarch64 | opt_ood | log18 | collect_products |  | 1.094 [1.071, 1.185] | 1.062 [0.891, 1.490] | 142.928 / 184.210 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood | log18 | vector_products |  | 1.130 [1.087, 1.196] | 1.146 [0.896, 1.341] | 144.484 / 203.687 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood | log18 | prepared_products |  | 1.119 [1.082, 1.180] | 1.115 [0.478, 1.184] | 143.637 / 177.505 | 4.000 | 4 / 69104 | regression |
| aarch64 | opt_ood_reuse | log10 | allocate |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.245 / 2.335 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood_reuse | log10 | scratch | yes | 0.826 [0.820, 0.833] | 0.830 [0.815, 0.849] | 1.856 / 1.953 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_ood_reuse | log10 | indexed_scratch |  | 0.859 [0.853, 0.865] | 0.860 [0.842, 0.902] | 1.930 / 2.044 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_ood_reuse | log16 | allocate | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 102.326 / 162.003 | 1.000 | 3 / 67312 | pass |
| aarch64 | opt_ood_reuse | log16 | scratch |  | 1.017 [0.983, 1.053] | 1.046 [0.981, 1.088] | 104.733 / 128.414 | 1.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ood_reuse | log16 | indexed_scratch |  | 1.030 [0.985, 1.182] | 1.011 [0.980, 1.300] | 104.293 / 199.603 | 1.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ood_reuse | log18 | allocate | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 141.713 / 167.258 | 4.000 | 3 / 68080 | pass |
| aarch64 | opt_ood_reuse | log18 | scratch |  | 1.040 [0.999, 1.059] | 1.052 [0.964, 1.179] | 145.443 / 175.137 | 4.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ood_reuse | log18 | indexed_scratch |  | 1.046 [1.002, 1.075] | 1.070 [0.928, 1.200] | 148.031 / 172.367 | 4.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log8_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 46.359 / 49.889 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | depth_first |  | 0.865 [0.856, 0.872] | 0.863 [0.775, 0.873] | 40.136 / 43.682 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | tiled_half |  | 0.659 [0.654, 0.662] | 0.666 [0.566, 0.675] | 30.512 / 32.780 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | half_depth | yes | 0.661 [0.653, 0.665] | 0.668 [0.573, 0.680] | 30.510 / 33.308 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 429.060 / 522.882 | 1.500 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log12_lanes8 | depth_first |  | 0.561 [0.528, 0.614] | 0.535 [0.490, 0.569] | 241.815 / 269.883 | 1.500 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log12_lanes8 | tiled_half |  | 0.634 [0.598, 0.657] | 0.651 [0.604, 0.703] | 272.034 / 323.091 | 1.500 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log12_lanes8 | half_depth | yes | 0.530 [0.500, 0.572] | 0.494 [0.460, 0.545] | 224.924 / 255.237 | 1.500 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1181.682 / 1385.418 | 12.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes8 | depth_first |  | 0.726 [0.687, 0.760] | 0.715 [0.651, 0.799] | 839.953 / 980.946 | 12.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes8 | tiled_half |  | 0.909 [0.813, 0.948] | 0.905 [0.836, 0.973] | 1039.760 / 1255.267 | 12.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes8 | half_depth | yes | 0.696 [0.660, 0.725] | 0.663 [0.599, 0.728] | 810.464 / 927.596 | 12.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes32 | production | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2608.021 / 3138.982 | 48.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes32 | depth_first |  | 0.945 [0.917, 0.990] | 0.934 [0.869, 1.036] | 2495.135 / 2962.984 | 48.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log15_lanes32 | tiled_half |  | 1.003 [0.943, 1.031] | 0.979 [0.907, 1.079] | 2586.104 / 3095.682 | 48.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log15_lanes32 | half_depth |  | 0.934 [0.880, 0.964] | 0.942 [0.846, 1.062] | 2397.365 / 2921.699 | 48.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | production | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 5103.688 / 7573.825 | 96.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log16_lanes32 | depth_first |  | 1.011 [0.961, 1.056] | 1.188 [0.765, 1.412] | 5144.938 / 8089.379 | 96.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | tiled_half |  | 1.008 [0.974, 1.060] | 0.944 [0.865, 1.648] | 5196.042 / 8444.264 | 96.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | half_depth |  | 0.977 [0.913, 1.011] | 1.070 [0.696, 1.303] | 4984.104 / 7289.650 | 96.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21528.146 / 29538.333 | 384.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log18_lanes32 | depth_first |  | 0.962 [0.942, 1.032] | 0.861 [0.799, 1.008] | 21412.312 / 24419.931 | 384.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | tiled_half |  | 0.976 [0.938, 0.997] | 0.918 [0.738, 1.143] | 20995.584 / 25896.228 | 384.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | half_depth | yes | 0.943 [0.909, 0.985] | 0.791 [0.753, 0.971] | 20542.020 / 23104.125 | 384.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log8_lanes1 | production | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.635 / 3.233 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | 1.361 [1.324, 1.395] | 1.254 [1.205, 1.399] | 3.574 / 4.112 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | tiled_half |  | 1.145 [1.121, 1.167] | 1.090 [0.972, 1.135] | 3.022 / 3.319 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | half_depth |  | 1.138 [1.116, 1.174] | 1.050 [0.979, 1.129] | 3.008 / 3.297 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_pack | logs9_9 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 33.745 / 38.816 | 0.016 | 2 / 17904 | pass |
| aarch64 | opt_pack | logs9_9 | single_write |  | 0.021 [0.021, 0.022] | 0.020 [0.017, 0.021] | 0.724 / 0.771 | 0.016 | 1 / 16384 | pass |
| aarch64 | opt_pack | logs9_9 | tiled_write | yes | 0.017 [0.017, 0.018] | 0.017 [0.015, 0.019] | 0.592 / 0.676 | 0.016 | 1 / 16384 | pass |
| aarch64 | opt_pack | logs16_14 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 194.900 / 277.279 | 2.000 | 2 / 2098672 | pass |
| aarch64 | opt_pack | logs16_14 | single_write |  | 0.507 [0.389, 0.526] | 0.441 [0.243, 0.468] | 93.827 / 97.649 | 2.000 | 1 / 2097152 | pass |
| aarch64 | opt_pack | logs16_14 | tiled_write | yes | 0.405 [0.292, 0.424] | 0.397 [0.265, 0.447] | 75.160 / 94.702 | 2.000 | 2 / 2098672 | pass |
| aarch64 | opt_pack | logs18_18 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 427.878 / 575.419 | 8.000 | 2 / 8390128 | pass |
| aarch64 | opt_pack | logs18_18 | single_write |  | 1.065 [0.900, 1.109] | 0.940 [0.707, 1.023] | 442.404 / 477.052 | 8.000 | 1 / 8388608 | inconclusive |
| aarch64 | opt_pack | logs18_18 | tiled_write | yes | 0.376 [0.343, 0.412] | 0.395 [0.364, 0.455] | 162.823 / 210.492 | 8.000 | 2 / 8390128 | pass |
| aarch64 | opt_packed_ood | logs9_9 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 60.454 / 98.823 | 0.031 | 15 / 50736 | pass |
| aarch64 | opt_packed_ood | logs9_9 | single_write_ood |  | 0.049 [0.047, 0.054] | 0.036 [0.028, 0.047] | 2.989 / 3.180 | 0.031 | 3 / 32784 | pass |
| aarch64 | opt_packed_ood | logs9_9 | tiled_indexed_ood | yes | 0.048 [0.046, 0.053] | 0.036 [0.028, 0.047] | 2.931 / 3.126 | 0.031 | 3 / 32784 | pass |
| aarch64 | opt_packed_ood | logs16_14 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 304.081 / 411.841 | 4.000 | 22 / 2231248 | pass |
| aarch64 | opt_packed_ood | logs16_14 | single_write_ood |  | 0.843 [0.818, 0.868] | 0.878 [0.759, 1.118] | 255.921 / 355.005 | 4.000 | 4 / 2164720 | inconclusive |
| aarch64 | opt_packed_ood | logs16_14 | tiled_indexed_ood | yes | 0.564 [0.485, 0.609] | 0.600 [0.445, 0.667] | 172.731 / 202.886 | 4.000 | 4 / 2164720 | pass |
| aarch64 | opt_packed_ood | logs18_18 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 609.904 / 753.461 | 16.000 | 24 / 8527312 | pass |
| aarch64 | opt_packed_ood | logs18_18 | single_write_ood |  | 1.166 [1.113, 1.215] | 1.124 [1.028, 1.156] | 714.917 / 796.005 | 16.000 | 4 / 8457712 | regression |
| aarch64 | opt_packed_ood | logs18_18 | tiled_indexed_ood | yes | 0.568 [0.541, 0.581] | 0.591 [0.551, 0.657] | 345.232 / 423.888 | 16.000 | 4 / 8457712 | pass |
