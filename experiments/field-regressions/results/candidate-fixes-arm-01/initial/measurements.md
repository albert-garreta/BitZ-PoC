# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_exact_signed_mac | l2_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.078 / 0.086 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n16 | fused_exact | yes | 0.792 [0.782, 0.809] | 0.795 [0.758, 0.809] | 0.062 / 0.067 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n16 | signed_columns |  | 1.067 [1.049, 1.077] | 1.054 [1.008, 1.092] | 0.083 / 0.091 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l2_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.932 / 5.279 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | fused_exact | yes | 0.787 [0.782, 0.790] | 0.784 [0.772, 0.797] | 3.854 / 4.157 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | signed_columns |  | 1.065 [1.059, 1.068] | 1.065 [1.046, 1.084] | 5.241 / 5.619 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l4_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.234 / 0.248 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | fused_exact |  | 0.966 [0.952, 0.976] | 0.968 [0.953, 0.989] | 0.225 / 0.240 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | signed_columns | yes | 0.720 [0.716, 0.735] | 0.726 [0.716, 0.748] | 0.170 / 0.182 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 14.512 / 15.504 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | fused_exact |  | 0.964 [0.958, 0.971] | 0.969 [0.944, 0.976] | 14.031 / 14.823 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | signed_columns | yes | 0.709 [0.703, 0.719] | 0.713 [0.700, 0.734] | 10.298 / 11.102 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.114 / 1.202 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n16 | fused_exact |  | 1.327 [1.318, 1.347] | 1.323 [1.298, 1.351] | 1.487 / 1.585 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l9_n16 | signed_columns | yes | 0.656 [0.650, 0.666] | 0.651 [0.636, 0.670] | 0.732 / 0.779 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 70.831 / 75.752 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n1024 | fused_exact |  | 1.315 [1.296, 1.324] | 1.302 [1.282, 1.326] | 92.787 / 99.410 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l9_n1024 | signed_columns | yes | 0.674 [0.667, 0.686] | 0.682 [0.669, 0.693] | 48.031 / 51.824 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 27.217 / 29.498 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_vartime |  | 0.964 [0.959, 0.975] | 0.967 [0.940, 0.991] | 26.311 / 28.580 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_ct |  | 0.977 [0.968, 0.984] | 0.974 [0.957, 0.996] | 26.543 / 29.052 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | public_nonzero | yes | 0.968 [0.964, 0.982] | 0.962 [0.939, 0.981] | 26.306 / 28.705 | 0.000 | 2 / 98288 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.364 / 23.216 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_vartime |  | 1.230 [1.223, 1.239] | 1.223 [1.203, 1.248] | 26.338 / 28.310 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_ct |  | 1.243 [1.233, 1.255] | 1.239 [1.213, 1.252] | 26.539 / 28.683 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | public_nonzero | yes | 0.857 [0.850, 0.864] | 0.858 [0.837, 0.878] | 18.322 / 19.764 | 0.000 | 2 / 98272 | pass |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.332 / 4.728 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_vartime |  | 6.075 [5.975, 6.126] | 5.945 [5.811, 6.026] | 26.281 / 28.186 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_ct |  | 6.169 [6.085, 6.217] | 6.039 [5.899, 6.103] | 26.752 / 28.472 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | public_nonzero | yes | 0.366 [0.357, 0.373] | 0.383 [0.366, 0.412] | 1.594 / 1.844 | 0.000 | 1 / 81920 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 27.164 / 28.906 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_vartime |  | 0.968 [0.959, 0.970] | 0.967 [0.947, 0.995] | 26.226 / 28.038 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_ct |  | 0.972 [0.964, 0.977] | 0.980 [0.960, 0.993] | 26.448 / 28.222 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | public_nonzero | yes | 0.965 [0.962, 0.973] | 0.968 [0.949, 0.985] | 26.256 / 27.938 | 0.000 | 2 / 98288 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.410 / 23.496 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_vartime |  | 1.229 [1.220, 1.245] | 1.218 [1.194, 1.255] | 26.461 / 28.646 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_ct |  | 1.242 [1.231, 1.252] | 1.211 [1.182, 1.259] | 26.650 / 28.726 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | public_nonzero | yes | 0.857 [0.847, 0.865] | 0.861 [0.838, 0.946] | 18.395 / 20.162 | 0.000 | 2 / 98272 | pass |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.401 / 4.754 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_vartime |  | 6.113 [6.012, 6.178] | 5.946 [5.859, 6.167] | 26.698 / 28.318 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_ct |  | 6.155 [6.059, 6.233] | 6.104 [5.935, 6.256] | 26.832 / 28.680 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | public_nonzero | yes | 0.360 [0.350, 0.367] | 0.386 [0.367, 0.415] | 1.589 / 1.812 | 0.000 | 1 / 81920 | pass |
| aarch64 | opt_ood | log10 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.079 / 3.336 | 0.016 | 13 / 32832 | pass |
| aarch64 | opt_ood | log10 | reuse_products |  | 0.723 [0.720, 0.730] | 0.721 [0.707, 0.738] | 2.227 / 2.405 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | indexed_products |  | 0.755 [0.750, 0.767] | 0.762 [0.744, 0.776] | 2.319 / 2.507 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | collect_products |  | 0.761 [0.755, 0.768] | 0.759 [0.744, 0.777] | 2.336 / 2.532 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | vector_products |  | 0.762 [0.752, 0.767] | 0.758 [0.740, 0.768] | 2.338 / 2.475 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | prepared_products |  | 0.752 [0.746, 0.760] | 0.759 [0.747, 0.775] | 2.314 / 2.504 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | tiled_1024 |  | 0.715 [0.709, 0.724] | 0.714 [0.698, 0.725] | 2.200 / 2.357 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_256 |  | 0.392 [0.388, 0.400] | 0.393 [0.385, 0.408] | 1.204 / 1.318 | 0.016 | 2 / 4160 | pass |
| aarch64 | opt_ood | log10 | vector_1024 |  | 0.754 [0.743, 0.760] | 0.750 [0.739, 0.770] | 2.311 / 2.485 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | vector_2048 |  | 0.750 [0.746, 0.763] | 0.739 [0.726, 0.762] | 2.311 / 2.484 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_collect |  | 0.723 [0.716, 0.741] | 0.725 [0.711, 0.740] | 2.218 / 2.403 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_grouped | yes | 0.716 [0.712, 0.727] | 0.711 [0.694, 0.730] | 2.202 / 2.348 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log16 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 66.305 / 76.471 | 1.000 | 20 / 133328 | pass |
| aarch64 | opt_ood | log16 | reuse_products |  | 0.886 [0.863, 0.909] | 0.918 [0.880, 0.969] | 58.582 / 71.230 | 1.000 | 3 / 67312 | pass |
| aarch64 | opt_ood | log16 | indexed_products |  | 0.960 [0.948, 0.990] | 0.991 [0.966, 1.084] | 63.979 / 77.225 | 1.000 | 3 / 67312 | inconclusive |
| aarch64 | opt_ood | log16 | collect_products |  | 0.968 [0.958, 0.992] | 0.978 [0.946, 1.021] | 63.928 / 74.076 | 1.000 | 4 / 67568 | inconclusive |
| aarch64 | opt_ood | log16 | vector_products |  | 0.983 [0.970, 1.003] | 1.016 [0.976, 1.063] | 65.225 / 77.161 | 1.000 | 4 / 67568 | inconclusive |
| aarch64 | opt_ood | log16 | prepared_products |  | 0.965 [0.954, 0.995] | 1.018 [0.967, 1.144] | 64.470 / 78.377 | 1.000 | 4 / 67568 | inconclusive |
| aarch64 | opt_ood | log16 | tiled_1024 |  | 0.708 [0.686, 0.716] | 0.669 [0.653, 0.698] | 46.941 / 51.114 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | tiled_256 |  | 0.707 [0.678, 0.715] | 0.669 [0.637, 0.699] | 46.715 / 49.844 | 1.000 | 2 / 8192 | pass |
| aarch64 | opt_ood | log16 | vector_1024 |  | 0.795 [0.755, 0.802] | 0.756 [0.717, 0.784] | 52.423 / 56.321 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | vector_2048 |  | 0.803 [0.772, 0.816] | 0.760 [0.731, 0.796] | 53.311 / 57.043 | 1.000 | 2 / 33280 | pass |
| aarch64 | opt_ood | log16 | tiled_collect |  | 0.707 [0.685, 0.717] | 0.669 [0.641, 0.694] | 46.803 / 50.361 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | tiled_grouped | yes | 0.704 [0.678, 0.716] | 0.673 [0.644, 0.697] | 46.666 / 50.347 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log18 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 219.546 / 253.624 | 4.000 | 22 / 135632 | pass |
| aarch64 | opt_ood | log18 | reuse_products |  | 0.901 [0.885, 0.915] | 0.886 [0.859, 0.906] | 196.578 / 223.345 | 4.000 | 3 / 68080 | pass |
| aarch64 | opt_ood | log18 | indexed_products |  | 0.986 [0.978, 1.011] | 0.993 [0.946, 1.015] | 218.018 / 246.353 | 4.000 | 3 / 68080 | inconclusive |
| aarch64 | opt_ood | log18 | collect_products |  | 1.001 [0.981, 1.015] | 0.991 [0.959, 1.016] | 218.574 / 247.702 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | vector_products |  | 1.012 [0.994, 1.038] | 1.001 [0.966, 1.035] | 219.855 / 251.861 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | prepared_products |  | 0.997 [0.983, 1.018] | 0.997 [0.956, 1.015] | 219.647 / 252.189 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | tiled_1024 |  | 0.833 [0.828, 0.844] | 0.803 [0.773, 0.824] | 182.698 / 200.135 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | tiled_256 |  | 0.853 [0.846, 0.863] | 0.820 [0.784, 0.853] | 188.035 / 203.358 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | vector_1024 |  | 0.935 [0.925, 0.943] | 0.898 [0.868, 0.924] | 205.064 / 223.471 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | vector_2048 |  | 0.937 [0.927, 0.946] | 0.912 [0.880, 0.932] | 206.057 / 225.952 | 4.000 | 2 / 34816 | pass |
| aarch64 | opt_ood | log18 | tiled_collect |  | 0.831 [0.825, 0.847] | 0.795 [0.770, 0.821] | 183.223 / 201.270 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | tiled_grouped | yes | 0.832 [0.826, 0.844] | 0.797 [0.780, 0.827] | 183.836 / 201.944 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ntt | log8_lanes1 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.654 / 3.034 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | 1.493 [1.441, 1.552] | 1.569 [1.444, 1.697] | 3.962 / 4.852 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | tiled_half |  | 1.170 [1.157, 1.188] | 1.162 [1.090, 1.244] | 3.106 / 3.600 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | half_depth |  | 1.169 [1.155, 1.183] | 1.124 [1.071, 1.240] | 3.102 / 3.506 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | incremental | yes | 0.447 [0.444, 0.462] | 0.429 [0.404, 0.446] | 1.189 / 1.289 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 46.508 / 50.797 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | depth_first |  | 0.865 [0.860, 0.875] | 0.858 [0.839, 0.885] | 40.255 / 43.398 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | tiled_half |  | 0.663 [0.659, 0.672] | 0.659 [0.646, 0.680] | 30.824 / 33.843 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | half_depth |  | 0.686 [0.674, 0.690] | 0.674 [0.658, 0.690] | 31.832 / 33.935 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | incremental | yes | 0.684 [0.671, 0.688] | 0.680 [0.664, 0.691] | 31.774 / 34.325 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 413.125 / 449.822 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | depth_first |  | 0.891 [0.882, 0.896] | 0.897 [0.875, 0.914] | 367.721 / 406.718 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | tiled_half |  | 0.805 [0.798, 0.811] | 0.817 [0.793, 0.833] | 332.325 / 364.519 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | half_depth |  | 0.810 [0.803, 0.813] | 0.805 [0.795, 0.834] | 333.656 / 366.280 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | incremental | yes | 0.806 [0.801, 0.813] | 0.810 [0.795, 0.833] | 334.823 / 365.732 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.836 / 0.904 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | chunked |  | 1.010 [1.006, 1.018] | 1.014 [0.996, 1.030] | 0.849 / 0.914 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_grid | 16 | direct_tiles |  | 0.978 [0.973, 0.984] | 0.987 [0.967, 1.000] | 0.820 / 0.888 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | direct_pass | yes | 0.981 [0.976, 0.990] | 0.983 [0.968, 0.995] | 0.823 / 0.894 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 53.654 / 58.506 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | chunked |  | 1.043 [1.035, 1.053] | 1.033 [1.018, 1.060] | 56.332 / 60.281 | 1.000 | 0 / 0 | regression |
| aarch64 | opt_gf_grid | 1024 | direct_tiles |  | 0.975 [0.967, 0.987] | 0.969 [0.953, 0.989] | 52.664 / 56.740 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | direct_pass | yes | 0.970 [0.964, 0.980] | 0.973 [0.946, 0.996] | 52.316 / 57.163 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.143 / 0.151 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | fixed_schedule |  | 0.610 [0.605, 0.617] | 0.618 [0.609, 0.640] | 0.087 / 0.094 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_fused |  | 1.101 [1.095, 1.122] | 1.115 [1.089, 1.140] | 0.157 / 0.169 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_adaptive |  | 0.626 [0.621, 0.635] | 0.628 [0.615, 0.647] | 0.089 / 0.095 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | row_density |  | 0.635 [0.630, 0.646] | 0.646 [0.629, 0.691] | 0.091 / 0.100 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_masks |  | 0.997 [0.992, 1.016] | 1.010 [0.989, 1.038] | 0.143 / 0.155 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_blocks8 |  | 0.747 [0.740, 0.760] | 0.748 [0.739, 0.775] | 0.107 / 0.115 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_blocks32 |  | 0.753 [0.749, 0.767] | 0.772 [0.757, 0.800] | 0.108 / 0.119 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_classified |  | 0.671 [0.663, 0.676] | 0.684 [0.660, 0.695] | 0.095 / 0.104 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_scan | yes | 0.869 [0.861, 0.879] | 0.873 [0.863, 0.897] | 0.124 / 0.134 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.424 / 9.157 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | fixed_schedule |  | 0.802 [0.795, 0.809] | 0.805 [0.771, 0.824] | 6.718 / 7.166 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_fused |  | 1.178 [1.161, 1.187] | 1.158 [1.131, 1.184] | 9.927 / 10.485 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_adaptive |  | 0.806 [0.795, 0.814] | 0.794 [0.772, 0.827] | 6.760 / 7.228 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | row_density |  | 0.674 [0.664, 0.678] | 0.678 [0.656, 0.686] | 5.611 / 6.073 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_masks |  | 1.094 [1.073, 1.100] | 1.084 [1.056, 1.109] | 9.162 / 9.840 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_blocks8 |  | 0.792 [0.778, 0.797] | 0.790 [0.770, 0.810] | 6.642 / 7.173 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_blocks32 |  | 0.941 [0.932, 0.953] | 0.935 [0.915, 0.958] | 7.936 / 8.469 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_classified |  | 0.703 [0.694, 0.712] | 0.699 [0.684, 0.727] | 5.894 / 6.392 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_scan | yes | 0.937 [0.931, 0.946] | 0.942 [0.918, 0.955] | 7.829 / 8.620 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.059 / 0.065 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | fixed_schedule |  | 1.506 [1.384, 1.618] | 1.579 [1.392, 1.616] | 0.087 / 0.095 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_fused |  | 0.864 [0.826, 0.904] | 0.848 [0.831, 0.910] | 0.050 / 0.056 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_adaptive |  | 0.872 [0.849, 0.914] | 0.871 [0.842, 0.926] | 0.051 / 0.057 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | row_density |  | 0.987 [0.944, 1.045] | 1.008 [0.948, 1.071] | 0.058 / 0.066 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_masks |  | 1.305 [0.968, 1.947] | 1.325 [0.967, 1.917] | 0.079 / 0.110 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_blocks8 |  | 0.986 [0.941, 1.009] | 0.968 [0.941, 1.022] | 0.057 / 0.063 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_blocks32 |  | 0.914 [0.868, 0.948] | 0.944 [0.871, 0.978] | 0.053 / 0.059 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_classified |  | 1.013 [0.961, 1.092] | 1.003 [0.939, 1.082] | 0.060 / 0.067 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_scan | yes | 0.878 [0.855, 0.922] | 0.909 [0.858, 0.938] | 0.052 / 0.057 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.609 / 4.027 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | fixed_schedule |  | 1.840 [1.766, 1.969] | 1.777 [1.713, 1.899] | 6.678 / 7.126 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_fused |  | 0.855 [0.841, 0.865] | 0.866 [0.830, 0.899] | 3.104 / 3.466 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_adaptive |  | 0.851 [0.839, 0.868] | 0.852 [0.813, 0.896] | 3.087 / 3.423 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | row_density |  | 1.476 [1.453, 1.500] | 1.440 [1.403, 1.484] | 5.350 / 5.835 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_masks |  | 1.089 [1.062, 1.119] | 1.068 [1.041, 1.101] | 3.926 / 4.297 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_blocks8 |  | 0.980 [0.970, 1.003] | 0.954 [0.937, 1.016] | 3.561 / 3.950 | 0.078 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_blocks32 |  | 0.882 [0.869, 0.899] | 0.878 [0.856, 0.909] | 3.187 / 3.607 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_classified |  | 1.480 [1.461, 1.515] | 1.434 [1.396, 1.509] | 5.366 / 5.904 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_scan | yes | 0.858 [0.838, 0.870] | 0.863 [0.822, 0.902] | 3.108 / 3.456 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.020 / 0.022 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | fixed_schedule |  | 4.272 [4.194, 4.304] | 4.205 [4.081, 4.276] | 0.087 / 0.093 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_fused |  | 0.932 [0.927, 0.941] | 0.947 [0.906, 0.958] | 0.019 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_adaptive |  | 0.964 [0.956, 0.974] | 0.975 [0.941, 0.983] | 0.020 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | row_density |  | 0.434 [0.431, 0.437] | 0.442 [0.422, 0.450] | 0.009 / 0.010 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_masks |  | 0.872 [0.860, 0.883] | 0.866 [0.844, 0.893] | 0.018 / 0.019 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_blocks8 |  | 1.262 [1.251, 1.267] | 1.270 [1.229, 1.287] | 0.026 / 0.028 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_blocks32 |  | 1.058 [1.047, 1.066] | 1.061 [1.027, 1.082] | 0.021 / 0.023 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_classified |  | 0.434 [0.430, 0.436] | 0.432 [0.425, 0.446] | 0.009 / 0.010 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_scan | yes | 0.473 [0.469, 0.476] | 0.476 [0.460, 0.482] | 0.010 / 0.011 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.119 / 1.208 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | fixed_schedule |  | 5.958 [5.885, 6.012] | 5.897 [5.743, 5.982] | 6.686 / 7.123 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_fused |  | 0.888 [0.869, 0.898] | 0.885 [0.872, 0.908] | 0.994 / 1.065 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_adaptive |  | 0.891 [0.872, 0.895] | 0.883 [0.872, 0.902] | 0.994 / 1.064 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | row_density |  | 0.403 [0.377, 0.409] | 0.405 [0.389, 0.421] | 0.452 / 0.490 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_masks |  | 0.838 [0.830, 0.851] | 0.846 [0.822, 0.857] | 0.949 / 1.020 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_blocks8 |  | 1.290 [1.265, 1.297] | 1.280 [1.254, 1.306] | 1.442 / 1.537 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_blocks32 |  | 0.991 [0.974, 0.996] | 0.983 [0.961, 1.014] | 1.110 / 1.188 | 0.078 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_classified |  | 0.375 [0.368, 0.378] | 0.380 [0.369, 0.387] | 0.419 / 0.452 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_scan | yes | 0.410 [0.402, 0.414] | 0.412 [0.406, 0.421] | 0.457 / 0.500 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.076 / 0.088 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | fixed_schedule |  | 1.196 [1.021, 1.367] | 1.170 [1.018, 1.371] | 0.087 / 0.092 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_fused |  | 0.938 [0.878, 0.974] | 0.956 [0.877, 0.970] | 0.071 / 0.082 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_adaptive |  | 1.220 [1.051, 1.404] | 1.195 [1.056, 1.400] | 0.089 / 0.096 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | row_density |  | 0.692 [0.606, 0.802] | 0.678 [0.607, 0.797] | 0.052 / 0.069 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_masks |  | 0.913 [0.745, 1.019] | 0.881 [0.761, 1.016] | 0.070 / 0.088 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_blocks8 |  | 1.044 [0.980, 1.094] | 1.048 [0.987, 1.097] | 0.082 / 0.090 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_blocks32 |  | 1.012 [0.927, 1.019] | 1.013 [0.924, 1.034] | 0.076 / 0.088 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_classified |  | 0.713 [0.612, 0.802] | 0.720 [0.614, 0.787] | 0.054 / 0.070 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_scan | yes | 0.976 [0.901, 1.009] | 0.961 [0.895, 1.011] | 0.074 / 0.083 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.691 / 4.056 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | fixed_schedule |  | 1.801 [1.774, 1.892] | 1.773 [1.706, 1.830] | 6.635 / 7.116 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_fused |  | 0.850 [0.837, 0.865] | 0.845 [0.807, 0.876] | 3.141 / 3.397 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_adaptive |  | 1.808 [1.776, 1.888] | 1.776 [1.692, 1.826] | 6.653 / 7.150 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | row_density |  | 1.460 [1.439, 1.481] | 1.420 [1.378, 1.469] | 5.385 / 5.781 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_masks |  | 1.099 [1.084, 1.139] | 1.092 [1.035, 1.122] | 4.085 / 4.443 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_blocks8 |  | 0.975 [0.965, 1.007] | 0.985 [0.937, 1.028] | 3.621 / 3.947 | 0.078 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_blocks32 |  | 0.878 [0.867, 0.896] | 0.905 [0.836, 0.931] | 3.254 / 3.606 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_classified |  | 1.454 [1.436, 1.487] | 1.383 [1.365, 1.457] | 5.352 / 5.726 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_scan | yes | 0.848 [0.836, 0.865] | 0.855 [0.812, 0.882] | 3.144 / 3.423 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.107 / 0.114 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | fixed_schedule |  | 0.806 [0.799, 0.811] | 0.798 [0.785, 0.817] | 0.086 / 0.091 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_fused |  | 1.112 [1.104, 1.120] | 1.098 [1.089, 1.130] | 0.119 / 0.128 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_adaptive |  | 1.106 [1.097, 1.113] | 1.099 [1.079, 1.120] | 0.119 / 0.126 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | row_density |  | 0.646 [0.642, 0.653] | 0.643 [0.633, 0.663] | 0.069 / 0.074 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_masks |  | 0.986 [0.978, 0.995] | 0.987 [0.970, 1.014] | 0.105 / 0.114 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_blocks8 |  | 0.908 [0.903, 0.919] | 0.900 [0.887, 0.918] | 0.098 / 0.104 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_blocks32 |  | 1.142 [1.131, 1.155] | 1.133 [1.118, 1.157] | 0.123 / 0.131 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_classified |  | 0.684 [0.681, 0.691] | 0.683 [0.670, 0.698] | 0.073 / 0.079 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_scan | yes | 0.898 [0.888, 0.904] | 0.899 [0.878, 0.920] | 0.096 / 0.103 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.243 / 8.878 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | fixed_schedule |  | 0.805 [0.799, 0.814] | 0.821 [0.796, 0.835] | 6.645 / 7.204 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_fused |  | 1.169 [1.162, 1.185] | 1.169 [1.144, 1.200] | 9.681 / 10.247 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_adaptive |  | 1.176 [1.162, 1.189] | 1.179 [1.141, 1.204] | 9.645 / 10.377 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | row_density |  | 0.675 [0.670, 0.683] | 0.678 [0.668, 0.697] | 5.559 / 6.073 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_masks |  | 1.088 [1.082, 1.095] | 1.099 [1.064, 1.114] | 8.972 / 9.669 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_blocks8 |  | 0.790 [0.785, 0.800] | 0.793 [0.775, 0.810] | 6.515 / 6.999 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_blocks32 |  | 0.950 [0.944, 0.959] | 0.944 [0.928, 0.970] | 7.829 / 8.337 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_classified |  | 0.711 [0.706, 0.718] | 0.710 [0.700, 0.734] | 5.865 / 6.357 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_scan | yes | 0.934 [0.929, 0.941] | 0.941 [0.916, 0.960] | 7.687 / 8.423 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.080 / 0.086 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | fixed_schedule |  | 1.084 [1.077, 1.098] | 1.072 [1.045, 1.094] | 0.087 / 0.092 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_fused |  | 1.018 [1.009, 1.029] | 1.023 [0.986, 1.040] | 0.081 / 0.088 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_adaptive |  | 1.014 [1.005, 1.024] | 1.022 [0.997, 1.048] | 0.081 / 0.087 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | row_density |  | 0.619 [0.613, 0.628] | 0.621 [0.604, 0.637] | 0.049 / 0.053 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_masks |  | 0.886 [0.880, 0.897] | 0.878 [0.863, 0.898] | 0.071 / 0.075 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_blocks8 |  | 1.112 [1.102, 1.125] | 1.099 [1.079, 1.127] | 0.088 / 0.094 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_blocks32 |  | 1.063 [1.055, 1.079] | 1.058 [1.042, 1.086] | 0.085 / 0.092 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_classified |  | 0.643 [0.637, 0.648] | 0.645 [0.626, 0.669] | 0.051 / 0.055 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_scan | yes | 0.864 [0.857, 0.873] | 0.849 [0.827, 0.874] | 0.069 / 0.074 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.668 / 4.996 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | fixed_schedule |  | 1.420 [1.412, 1.437] | 1.409 [1.393, 1.440] | 6.663 / 7.068 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_fused |  | 1.054 [1.044, 1.062] | 1.036 [1.019, 1.076] | 4.904 / 5.265 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_adaptive |  | 1.051 [1.043, 1.061] | 1.048 [1.034, 1.070] | 4.911 / 5.223 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | row_density |  | 0.644 [0.640, 0.652] | 0.650 [0.636, 0.660] | 3.019 / 3.225 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_masks |  | 0.971 [0.963, 0.979] | 0.979 [0.955, 0.993] | 4.520 / 4.855 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_blocks8 |  | 1.131 [1.120, 1.140] | 1.131 [1.110, 1.146] | 5.290 / 5.616 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_blocks32 |  | 1.071 [1.063, 1.088] | 1.078 [1.050, 1.109] | 4.996 / 5.428 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_classified |  | 0.669 [0.664, 0.681] | 0.666 [0.651, 0.686] | 3.116 / 3.344 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_scan | yes | 0.917 [0.907, 0.923] | 0.909 [0.892, 0.935] | 4.265 / 4.562 | 0.078 | 0 / 0 | pass |
