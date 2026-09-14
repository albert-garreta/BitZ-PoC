# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_exact_signed_mac | l2_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.077 / 0.083 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n16 | fused_exact | yes | 0.803 [0.797, 0.818] | 0.810 [0.779, 0.825] | 0.062 / 0.067 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n16 | signed_columns |  | 0.813 [0.806, 0.822] | 0.815 [0.782, 0.834] | 0.063 / 0.069 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.843 / 6.356 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | fused_exact | yes | 0.781 [0.773, 0.789] | 0.796 [0.777, 0.946] | 3.815 / 6.296 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | signed_columns |  | 0.788 [0.781, 0.798] | 0.799 [0.775, 0.825] | 3.853 / 5.086 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.228 / 0.241 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | fused_exact |  | 0.970 [0.962, 0.977] | 0.964 [0.912, 0.974] | 0.220 / 0.230 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | signed_columns | yes | 0.770 [0.759, 0.778] | 0.772 [0.756, 0.785] | 0.175 / 0.183 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 14.192 / 14.868 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | fused_exact |  | 0.968 [0.963, 0.978] | 0.970 [0.954, 0.978] | 13.805 / 14.428 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | signed_columns | yes | 0.759 [0.753, 0.763] | 0.764 [0.756, 0.778] | 10.760 / 11.368 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.098 / 1.146 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n16 | fused_exact |  | 1.330 [1.324, 1.340] | 1.327 [1.315, 1.340] | 1.457 / 1.546 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l9_n16 | signed_columns | yes | 0.692 [0.686, 0.698] | 0.700 [0.686, 0.720] | 0.758 / 0.812 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 70.068 / 76.543 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n1024 | fused_exact |  | 1.308 [1.295, 1.320] | 1.301 [1.291, 1.329] | 91.515 / 99.352 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l9_n1024 | signed_columns | yes | 0.683 [0.673, 0.691] | 0.692 [0.682, 0.703] | 47.965 / 51.369 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 27.016 / 30.454 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_vartime |  | 0.963 [0.957, 0.979] | 0.967 [0.930, 1.072] | 26.133 / 30.023 | 0.000 | 2 / 98304 | inconclusive |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_ct |  | 0.981 [0.965, 0.989] | 0.979 [0.958, 1.389] | 26.456 / 30.936 | 0.000 | 2 / 98304 | inconclusive |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | public_nonzero | yes | 0.971 [0.953, 0.980] | 0.970 [0.923, 1.036] | 26.220 / 29.631 | 0.000 | 2 / 98288 | inconclusive |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.350 / 26.173 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_vartime |  | 1.228 [1.207, 1.236] | 1.231 [1.024, 1.329] | 26.026 / 37.380 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_ct |  | 1.251 [1.233, 1.268] | 1.244 [1.115, 1.393] | 26.551 / 38.062 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | public_nonzero | yes | 0.853 [0.842, 0.864] | 0.863 [0.524, 0.903] | 18.296 / 25.571 | 0.000 | 2 / 98272 | pass |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.245 / 4.529 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_vartime |  | 6.065 [5.995, 6.083] | 5.922 [5.701, 6.057] | 25.634 / 26.748 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_ct |  | 6.136 [6.109, 6.200] | 6.056 [5.785, 6.192] | 26.161 / 27.076 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | public_nonzero | yes | 0.361 [0.356, 0.365] | 0.390 [0.360, 0.403] | 1.532 / 1.768 | 0.000 | 1 / 81920 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 26.869 / 28.638 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_vartime |  | 0.966 [0.957, 0.971] | 0.964 [0.954, 0.983] | 25.931 / 27.787 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_ct |  | 0.970 [0.963, 0.978] | 0.975 [0.968, 0.996] | 26.081 / 28.011 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | public_nonzero | yes | 0.964 [0.955, 0.973] | 0.970 [0.961, 0.986] | 25.876 / 27.869 | 0.000 | 2 / 98288 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.209 / 22.387 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_vartime |  | 1.225 [1.215, 1.230] | 1.229 [1.209, 1.251] | 25.979 / 27.522 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_ct |  | 1.236 [1.223, 1.240] | 1.234 [1.212, 1.243] | 26.178 / 27.247 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | public_nonzero | yes | 0.857 [0.849, 0.861] | 0.862 [0.841, 0.870] | 18.197 / 19.060 | 0.000 | 2 / 98272 | pass |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.233 / 4.460 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_vartime |  | 6.140 [6.029, 6.174] | 6.068 [5.879, 6.119] | 26.009 / 26.702 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_ct |  | 6.201 [6.106, 6.218] | 6.077 [5.903, 6.261] | 26.248 / 26.862 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | public_nonzero | yes | 0.361 [0.353, 0.365] | 0.381 [0.357, 0.406] | 1.525 / 1.673 | 0.000 | 1 / 81920 | pass |
| aarch64 | opt_ood | log10 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.059 / 3.185 | 0.016 | 13 / 32832 | pass |
| aarch64 | opt_ood | log10 | reuse_products |  | 0.725 [0.720, 0.729] | 0.732 [0.713, 0.740] | 2.220 / 2.313 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | indexed_products |  | 0.756 [0.749, 0.760] | 0.759 [0.742, 0.767] | 2.310 / 2.408 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | collect_products |  | 0.757 [0.753, 0.766] | 0.763 [0.748, 0.775] | 2.323 / 2.416 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | vector_products |  | 0.759 [0.754, 0.774] | 0.766 [0.749, 0.784] | 2.337 / 2.440 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | prepared_products |  | 0.749 [0.747, 0.755] | 0.756 [0.741, 0.766] | 2.302 / 2.393 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | tiled_1024 |  | 0.718 [0.711, 0.721] | 0.723 [0.704, 0.730] | 2.192 / 2.274 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_256 |  | 0.393 [0.390, 0.400] | 0.406 [0.390, 0.415] | 1.211 / 1.298 | 0.016 | 2 / 4160 | pass |
| aarch64 | opt_ood | log10 | vector_1024 |  | 0.751 [0.746, 0.760] | 0.758 [0.744, 0.767] | 2.304 / 2.406 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | vector_2048 |  | 0.751 [0.747, 0.757] | 0.755 [0.741, 0.766] | 2.305 / 2.404 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_collect |  | 0.725 [0.720, 0.729] | 0.729 [0.713, 0.746] | 2.221 / 2.322 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_grouped | yes | 0.714 [0.709, 0.719] | 0.720 [0.705, 0.730] | 2.187 / 2.295 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log16 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 67.065 / 74.447 | 1.000 | 20 / 133328 | pass |
| aarch64 | opt_ood | log16 | reuse_products |  | 0.887 [0.875, 0.893] | 0.851 [0.797, 0.935] | 59.182 / 65.734 | 1.000 | 3 / 67312 | pass |
| aarch64 | opt_ood | log16 | indexed_products |  | 0.961 [0.950, 0.985] | 0.971 [0.901, 1.105] | 65.473 / 72.229 | 1.000 | 3 / 67312 | inconclusive |
| aarch64 | opt_ood | log16 | collect_products |  | 0.958 [0.950, 0.989] | 0.959 [0.891, 0.995] | 64.933 / 70.956 | 1.000 | 4 / 67568 | pass |
| aarch64 | opt_ood | log16 | vector_products |  | 0.972 [0.958, 0.992] | 0.976 [0.921, 1.058] | 65.354 / 72.673 | 1.000 | 4 / 67568 | inconclusive |
| aarch64 | opt_ood | log16 | prepared_products |  | 0.957 [0.949, 0.979] | 0.957 [0.921, 1.027] | 64.524 / 73.235 | 1.000 | 4 / 67568 | inconclusive |
| aarch64 | opt_ood | log16 | tiled_1024 |  | 0.703 [0.676, 0.710] | 0.669 [0.607, 0.695] | 46.725 / 49.120 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | tiled_256 |  | 0.707 [0.676, 0.710] | 0.669 [0.594, 0.687] | 46.907 / 48.709 | 1.000 | 2 / 8192 | pass |
| aarch64 | opt_ood | log16 | vector_1024 |  | 0.786 [0.757, 0.794] | 0.742 [0.691, 0.769] | 52.346 / 54.223 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | vector_2048 |  | 0.797 [0.768, 0.805] | 0.748 [0.683, 0.775] | 53.170 / 55.025 | 1.000 | 2 / 33280 | pass |
| aarch64 | opt_ood | log16 | tiled_collect |  | 0.705 [0.681, 0.708] | 0.660 [0.592, 0.682] | 46.857 / 48.542 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | tiled_grouped | yes | 0.705 [0.678, 0.711] | 0.662 [0.597, 0.684] | 46.907 / 48.409 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log18 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 220.419 / 236.966 | 4.000 | 22 / 135632 | pass |
| aarch64 | opt_ood | log18 | reuse_products |  | 0.882 [0.876, 0.903] | 0.909 [0.876, 0.925] | 196.591 / 212.818 | 4.000 | 3 / 68080 | pass |
| aarch64 | opt_ood | log18 | indexed_products |  | 0.981 [0.973, 0.998] | 1.005 [0.967, 1.023] | 217.310 / 235.108 | 4.000 | 3 / 68080 | inconclusive |
| aarch64 | opt_ood | log18 | collect_products |  | 0.979 [0.971, 0.992] | 0.989 [0.948, 1.017] | 216.607 / 233.131 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | vector_products |  | 0.999 [0.990, 1.008] | 0.997 [0.978, 1.034] | 220.350 / 238.864 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | prepared_products |  | 0.985 [0.974, 0.999] | 0.991 [0.969, 1.020] | 217.756 / 234.617 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | tiled_1024 |  | 0.826 [0.813, 0.832] | 0.812 [0.787, 0.822] | 181.512 / 190.301 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | tiled_256 |  | 0.843 [0.831, 0.852] | 0.818 [0.804, 0.841] | 185.582 / 193.833 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | vector_1024 |  | 0.928 [0.916, 0.934] | 0.915 [0.884, 0.936] | 204.020 / 215.784 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | vector_2048 |  | 0.932 [0.917, 0.946] | 0.907 [0.885, 0.934] | 205.544 / 215.001 | 4.000 | 2 / 34816 | pass |
| aarch64 | opt_ood | log18 | tiled_collect |  | 0.826 [0.813, 0.831] | 0.820 [0.783, 0.833] | 181.126 / 190.410 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | tiled_grouped | yes | 0.827 [0.813, 0.833] | 0.815 [0.790, 0.827] | 181.858 / 190.442 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ntt | log8_lanes1 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.589 / 2.888 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | 1.502 [1.455, 1.575] | 1.675 [1.568, 1.754] | 3.979 / 4.665 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | tiled_half |  | 1.151 [1.141, 1.168] | 1.190 [1.159, 1.274] | 2.987 / 3.499 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | half_depth |  | 1.148 [1.140, 1.174] | 1.222 [1.161, 1.251] | 2.985 / 3.498 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | incremental | yes | 0.462 [0.449, 0.464] | 0.471 [0.450, 0.486] | 1.188 / 1.308 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 46.496 / 52.391 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | depth_first |  | 0.863 [0.852, 0.869] | 0.857 [0.819, 0.875] | 40.118 / 44.774 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | tiled_half |  | 0.659 [0.647, 0.666] | 0.669 [0.652, 0.692] | 30.710 / 33.317 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | half_depth |  | 0.661 [0.648, 0.666] | 0.671 [0.650, 0.686] | 30.766 / 34.592 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | incremental | yes | 0.661 [0.630, 0.666] | 0.656 [0.607, 0.676] | 30.636 / 33.192 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 411.643 / 444.634 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | depth_first |  | 0.895 [0.889, 0.905] | 0.893 [0.813, 0.907] | 368.779 / 398.295 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | tiled_half |  | 0.802 [0.798, 0.808] | 0.802 [0.760, 0.815] | 329.875 / 358.757 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | half_depth |  | 0.801 [0.793, 0.806] | 0.813 [0.753, 0.837] | 329.875 / 360.459 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | incremental | yes | 0.800 [0.794, 0.806] | 0.807 [0.679, 0.820] | 328.883 / 351.595 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.832 / 0.882 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | chunked |  | 1.008 [1.002, 1.015] | 1.002 [0.962, 1.022] | 0.840 / 0.879 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_grid | 16 | direct_tiles |  | 0.976 [0.969, 0.984] | 0.970 [0.944, 0.991] | 0.813 / 0.853 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | direct_pass | yes | 0.979 [0.971, 0.987] | 0.985 [0.940, 1.008] | 0.814 / 0.864 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 53.539 / 56.176 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | chunked |  | 1.046 [1.032, 1.054] | 1.040 [1.015, 1.051] | 55.929 / 58.223 | 1.000 | 0 / 0 | regression |
| aarch64 | opt_gf_grid | 1024 | direct_tiles |  | 0.970 [0.960, 0.975] | 0.967 [0.956, 0.982] | 51.918 / 54.367 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | direct_pass | yes | 0.969 [0.959, 0.973] | 0.976 [0.940, 0.984] | 51.795 / 53.750 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.141 / 0.145 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | fixed_schedule |  | 0.609 [0.606, 0.612] | 0.612 [0.606, 0.618] | 0.086 / 0.089 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_fused |  | 1.102 [1.094, 1.109] | 1.105 [1.091, 1.115] | 0.155 / 0.160 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_adaptive |  | 0.626 [0.623, 0.628] | 0.628 [0.622, 0.641] | 0.088 / 0.092 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | row_density |  | 0.633 [0.628, 0.635] | 0.636 [0.627, 0.645] | 0.089 / 0.092 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_masks |  | 0.996 [0.990, 1.000] | 0.988 [0.978, 1.006] | 0.140 / 0.145 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_blocks8 |  | 0.746 [0.742, 0.750] | 0.748 [0.739, 0.764] | 0.105 / 0.109 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_blocks32 |  | 0.760 [0.758, 0.768] | 0.758 [0.753, 0.772] | 0.107 / 0.111 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_classified |  | 0.669 [0.665, 0.674] | 0.678 [0.667, 0.692] | 0.094 / 0.099 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_scan | yes | 0.865 [0.860, 0.872] | 0.866 [0.855, 0.871] | 0.122 / 0.126 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.283 / 8.744 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | fixed_schedule |  | 0.796 [0.771, 0.803] | 0.804 [0.774, 0.822] | 6.584 / 6.858 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_fused |  | 1.175 [1.151, 1.180] | 1.181 [1.153, 1.191] | 9.746 / 10.220 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_adaptive |  | 0.804 [0.775, 0.809] | 0.803 [0.773, 0.816] | 6.654 / 6.852 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | row_density |  | 0.671 [0.650, 0.674] | 0.679 [0.653, 0.690] | 5.544 / 5.793 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_masks |  | 1.089 [1.051, 1.092] | 1.082 [1.045, 1.097] | 8.981 / 9.369 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_blocks8 |  | 0.789 [0.763, 0.791] | 0.785 [0.767, 0.802] | 6.509 / 6.811 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_blocks32 |  | 0.935 [0.915, 0.941] | 0.942 [0.910, 0.955] | 7.741 / 8.058 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_classified |  | 0.701 [0.678, 0.705] | 0.709 [0.680, 0.720] | 5.800 / 6.022 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_scan | yes | 0.934 [0.899, 0.939] | 0.921 [0.898, 0.950] | 7.721 / 7.984 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.056 / 0.063 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | fixed_schedule |  | 1.528 [1.384, 1.624] | 1.498 [1.377, 1.620] | 0.085 / 0.089 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_fused |  | 0.867 [0.835, 0.887] | 0.884 [0.831, 0.893] | 0.047 / 0.056 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_adaptive |  | 0.876 [0.837, 0.898] | 0.877 [0.842, 0.911] | 0.048 / 0.057 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | row_density |  | 0.984 [0.972, 1.087] | 0.998 [0.966, 1.095] | 0.058 / 0.065 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_masks |  | 1.284 [0.985, 1.943] | 1.284 [0.987, 1.930] | 0.079 / 0.103 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_blocks8 |  | 0.995 [0.951, 1.010] | 0.989 [0.969, 1.019] | 0.055 / 0.063 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_blocks32 |  | 0.911 [0.878, 0.936] | 0.912 [0.880, 0.950] | 0.050 / 0.059 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_classified |  | 1.001 [0.957, 1.076] | 0.997 [0.970, 1.081] | 0.057 / 0.068 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_scan | yes | 0.883 [0.858, 0.921] | 0.892 [0.870, 0.919] | 0.050 / 0.057 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.543 / 3.912 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | fixed_schedule |  | 1.858 [1.772, 1.951] | 1.827 [1.639, 1.914] | 6.565 / 6.782 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_fused |  | 0.853 [0.844, 0.860] | 0.822 [0.806, 0.876] | 3.025 / 3.272 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_adaptive |  | 0.850 [0.840, 0.858] | 0.822 [0.793, 0.863] | 3.001 / 3.240 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | row_density |  | 1.489 [1.465, 1.497] | 1.403 [1.376, 1.501] | 5.244 / 5.578 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_masks |  | 1.090 [1.068, 1.114] | 1.051 [1.000, 1.114] | 3.882 / 4.103 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_blocks8 |  | 0.986 [0.973, 0.994] | 0.973 [0.919, 1.001] | 3.493 / 3.726 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_blocks32 |  | 0.887 [0.873, 0.892] | 0.878 [0.831, 0.903] | 3.137 / 3.365 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_classified |  | 1.484 [1.459, 1.498] | 1.413 [1.363, 1.481] | 5.243 / 5.563 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_scan | yes | 0.852 [0.844, 0.859] | 0.843 [0.801, 0.876] | 3.022 / 3.253 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.020 / 0.034 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | fixed_schedule |  | 4.268 [4.236, 4.452] | 4.229 [4.165, 7.491] | 0.086 / 0.161 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_fused |  | 0.929 [0.904, 0.932] | 0.926 [0.577, 0.941] | 0.019 / 0.025 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_adaptive |  | 0.968 [0.957, 0.982] | 0.953 [0.945, 1.099] | 0.020 / 0.035 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | row_density |  | 0.436 [0.402, 0.437] | 0.435 [0.292, 0.478] | 0.009 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_masks |  | 0.876 [0.867, 0.885] | 0.877 [0.858, 0.973] | 0.018 / 0.029 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_blocks8 |  | 1.282 [1.271, 1.290] | 1.283 [1.258, 1.659] | 0.026 / 0.046 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_blocks32 |  | 1.074 [1.061, 1.150] | 1.069 [1.041, 2.111] | 0.022 / 0.041 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_classified |  | 0.434 [0.418, 0.437] | 0.439 [0.425, 0.520] | 0.009 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_scan | yes | 0.473 [0.444, 0.477] | 0.474 [0.425, 0.579] | 0.010 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.110 / 1.159 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | fixed_schedule |  | 5.936 [5.911, 6.003] | 5.914 [5.811, 5.961] | 6.623 / 6.832 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_fused |  | 0.885 [0.878, 0.890] | 0.890 [0.877, 0.897] | 0.984 / 1.030 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_adaptive |  | 0.888 [0.880, 0.894] | 0.891 [0.881, 0.902] | 0.986 / 1.042 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | row_density |  | 0.405 [0.402, 0.408] | 0.411 [0.401, 0.415] | 0.450 / 0.477 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_masks |  | 0.844 [0.838, 0.851] | 0.847 [0.836, 0.859] | 0.938 / 0.981 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_blocks8 |  | 1.288 [1.281, 1.302] | 1.292 [1.271, 1.306] | 1.434 / 1.503 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_blocks32 |  | 0.989 [0.976, 0.996] | 0.987 [0.976, 0.997] | 1.094 / 1.141 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_classified |  | 0.375 [0.372, 0.378] | 0.377 [0.372, 0.382] | 0.416 / 0.438 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_scan | yes | 0.409 [0.405, 0.412] | 0.414 [0.409, 0.429] | 0.455 / 0.484 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.072 / 0.085 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | fixed_schedule |  | 1.201 [1.022, 1.371] | 1.202 [1.019, 1.354] | 0.086 / 0.088 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_fused |  | 0.964 [0.884, 0.992] | 0.960 [0.887, 0.991] | 0.071 / 0.079 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_adaptive |  | 1.239 [1.055, 1.406] | 1.243 [1.066, 1.391] | 0.089 / 0.091 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | row_density |  | 0.708 [0.605, 0.827] | 0.717 [0.608, 0.825] | 0.051 / 0.068 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_masks |  | 0.905 [0.750, 1.015] | 0.902 [0.754, 1.028] | 0.067 / 0.086 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_blocks8 |  | 1.066 [0.982, 1.097] | 1.056 [0.987, 1.095] | 0.079 / 0.087 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_blocks32 |  | 1.026 [0.936, 1.034] | 1.032 [0.936, 1.043] | 0.074 / 0.085 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_classified |  | 0.724 [0.616, 0.820] | 0.731 [0.622, 0.816] | 0.052 / 0.068 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_scan | yes | 0.996 [0.912, 1.020] | 0.997 [0.923, 1.022] | 0.072 / 0.082 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.630 / 4.003 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | fixed_schedule |  | 1.835 [1.776, 1.876] | 1.713 [1.631, 1.792] | 6.582 / 6.953 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_fused |  | 0.852 [0.838, 0.862] | 0.829 [0.741, 0.853] | 3.100 / 3.297 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_adaptive |  | 1.844 [1.788, 1.879] | 1.739 [1.654, 1.812] | 6.612 / 6.948 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | row_density |  | 1.469 [1.447, 1.495] | 1.400 [1.260, 1.449] | 5.282 / 5.604 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_masks |  | 1.112 [1.078, 1.157] | 1.058 [0.913, 1.109] | 4.031 / 4.327 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_blocks8 |  | 0.987 [0.976, 0.998] | 0.949 [0.876, 0.981] | 3.574 / 3.783 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_blocks32 |  | 0.886 [0.872, 0.895] | 0.858 [0.825, 0.882] | 3.217 / 3.401 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_classified |  | 1.472 [1.434, 1.498] | 1.395 [1.243, 1.432] | 5.278 / 5.627 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_scan | yes | 0.852 [0.841, 0.866] | 0.818 [0.766, 0.853] | 3.114 / 3.351 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.107 / 0.111 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | fixed_schedule |  | 0.806 [0.799, 0.813] | 0.796 [0.782, 0.815] | 0.086 / 0.089 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_fused |  | 1.116 [1.105, 1.123] | 1.108 [1.093, 1.131] | 0.119 / 0.123 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_adaptive |  | 1.110 [1.098, 1.117] | 1.087 [1.073, 1.131] | 0.118 / 0.122 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | row_density |  | 0.646 [0.642, 0.650] | 0.645 [0.629, 0.655] | 0.069 / 0.071 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_masks |  | 0.988 [0.980, 0.992] | 0.977 [0.965, 1.013] | 0.105 / 0.109 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_blocks8 |  | 0.912 [0.907, 0.920] | 0.918 [0.896, 0.937] | 0.097 / 0.101 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_blocks32 |  | 1.148 [1.135, 1.157] | 1.137 [1.116, 1.162] | 0.122 / 0.126 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_classified |  | 0.684 [0.678, 0.690] | 0.685 [0.669, 0.692] | 0.073 / 0.076 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_scan | yes | 0.895 [0.889, 0.902] | 0.890 [0.872, 0.907] | 0.095 / 0.099 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.220 / 8.564 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | fixed_schedule |  | 0.803 [0.783, 0.808] | 0.801 [0.784, 0.812] | 6.583 / 6.787 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_fused |  | 1.173 [1.152, 1.176] | 1.167 [1.157, 1.193] | 9.629 / 9.955 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_adaptive |  | 1.171 [1.154, 1.180] | 1.169 [1.149, 1.180] | 9.620 / 9.909 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | row_density |  | 0.674 [0.660, 0.677] | 0.671 [0.657, 0.684] | 5.530 / 5.683 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_masks |  | 1.089 [1.060, 1.096] | 1.087 [1.057, 1.095] | 8.933 / 9.153 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_blocks8 |  | 0.791 [0.773, 0.794] | 0.794 [0.771, 0.802] | 6.487 / 6.687 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_blocks32 |  | 0.948 [0.926, 0.955] | 0.951 [0.928, 0.962] | 7.782 / 8.039 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_classified |  | 0.677 [0.654, 0.681] | 0.679 [0.662, 0.684] | 5.547 / 5.718 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_scan | yes | 0.936 [0.905, 0.939] | 0.933 [0.906, 0.941] | 7.654 / 7.867 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.079 / 0.081 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | fixed_schedule |  | 1.093 [1.087, 1.097] | 1.094 [1.068, 1.105] | 0.086 / 0.089 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_fused |  | 1.023 [1.014, 1.027] | 1.016 [0.997, 1.031] | 0.081 / 0.082 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_adaptive |  | 1.019 [1.015, 1.027] | 1.016 [1.002, 1.033] | 0.081 / 0.083 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | row_density |  | 0.622 [0.620, 0.626] | 0.625 [0.613, 0.635] | 0.049 / 0.051 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_masks |  | 0.889 [0.885, 0.895] | 0.892 [0.875, 0.919] | 0.070 / 0.073 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_blocks8 |  | 1.114 [1.109, 1.121] | 1.114 [1.091, 1.121] | 0.088 / 0.090 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_blocks32 |  | 1.065 [1.061, 1.074] | 1.067 [1.046, 1.089] | 0.084 / 0.087 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_classified |  | 0.643 [0.639, 0.646] | 0.647 [0.632, 0.674] | 0.051 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_scan | yes | 0.865 [0.859, 0.871] | 0.870 [0.851, 0.897] | 0.068 / 0.070 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.674 / 4.796 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | fixed_schedule |  | 1.416 [1.405, 1.423] | 1.405 [1.396, 1.424] | 6.602 / 6.767 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_fused |  | 1.052 [1.044, 1.059] | 1.053 [1.045, 1.073] | 4.906 / 5.093 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_adaptive |  | 1.049 [1.043, 1.055] | 1.053 [1.039, 1.062] | 4.899 / 5.026 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | row_density |  | 0.641 [0.638, 0.647] | 0.650 [0.642, 0.656] | 2.993 / 3.108 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_masks |  | 0.966 [0.958, 0.972] | 0.972 [0.959, 0.984] | 4.505 / 4.657 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_blocks8 |  | 1.125 [1.120, 1.132] | 1.123 [1.111, 1.141] | 5.257 / 5.387 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_blocks32 |  | 1.068 [1.064, 1.074] | 1.069 [1.057, 1.079] | 4.995 / 5.115 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_classified |  | 0.667 [0.662, 0.670] | 0.672 [0.664, 0.682] | 3.111 / 3.215 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_scan | yes | 0.913 [0.906, 0.918] | 0.913 [0.906, 0.922] | 4.259 / 4.388 | 0.078 | 0 / 0 | pass |
