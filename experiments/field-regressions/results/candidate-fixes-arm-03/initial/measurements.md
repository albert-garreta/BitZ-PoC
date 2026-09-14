# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_exact_signed_mac | l2_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.076 / 0.081 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n16 | fused_exact | yes | 0.786 [0.780, 0.801] | 0.803 [0.786, 0.814] | 0.061 / 0.064 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n16 | signed_columns |  | 0.814 [0.809, 0.820] | 0.817 [0.803, 0.834] | 0.062 / 0.066 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.766 / 5.059 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | fused_exact | yes | 0.785 [0.779, 0.793] | 0.790 [0.775, 0.800] | 3.738 / 3.993 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | signed_columns |  | 0.795 [0.788, 0.801] | 0.799 [0.792, 0.810] | 3.788 / 4.008 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.224 / 0.234 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | fused_exact |  | 0.967 [0.950, 0.975] | 0.956 [0.943, 0.978] | 0.215 / 0.225 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | signed_columns | yes | 0.759 [0.751, 0.770] | 0.757 [0.734, 0.767] | 0.171 / 0.181 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 14.004 / 14.441 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | fused_exact |  | 0.964 [0.956, 0.971] | 0.971 [0.961, 0.980] | 13.541 / 14.005 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | signed_columns | yes | 0.759 [0.749, 0.762] | 0.766 [0.756, 0.772] | 10.617 / 11.072 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.076 / 1.105 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n16 | fused_exact |  | 1.340 [1.329, 1.350] | 1.327 [1.319, 1.356] | 1.441 / 1.468 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l9_n16 | signed_columns | yes | 0.694 [0.690, 0.703] | 0.699 [0.694, 0.706] | 0.747 / 0.775 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 68.480 / 70.854 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n1024 | fused_exact |  | 1.324 [1.310, 1.330] | 1.306 [1.296, 1.323] | 90.320 / 92.781 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l9_n1024 | signed_columns | yes | 0.690 [0.680, 0.696] | 0.696 [0.681, 0.706] | 46.981 / 49.016 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 26.388 / 27.299 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_vartime |  | 0.966 [0.960, 0.974] | 0.967 [0.959, 0.981] | 25.509 / 26.421 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_ct |  | 0.975 [0.967, 0.980] | 0.977 [0.965, 0.983] | 25.750 / 26.630 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | public_nonzero | yes | 0.965 [0.958, 0.969] | 0.964 [0.958, 0.982] | 25.476 / 26.438 | 0.000 | 2 / 98288 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.029 / 21.937 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_vartime |  | 1.220 [1.214, 1.234] | 1.223 [1.201, 1.231] | 25.703 / 26.778 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_ct |  | 1.238 [1.227, 1.250] | 1.239 [1.214, 1.244] | 26.050 / 26.978 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | public_nonzero | yes | 0.848 [0.843, 0.857] | 0.853 [0.840, 0.872] | 17.815 / 18.778 | 0.000 | 2 / 98272 | pass |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.204 / 4.387 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_vartime |  | 6.059 [6.017, 6.094] | 6.011 [5.887, 6.189] | 25.500 / 26.408 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_ct |  | 6.188 [6.152, 6.214] | 6.067 [5.906, 6.211] | 26.009 / 26.996 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | public_nonzero | yes | 0.362 [0.356, 0.370] | 0.386 [0.370, 0.408] | 1.531 / 1.740 | 0.000 | 1 / 81920 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 26.560 / 27.540 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_vartime |  | 0.970 [0.955, 0.976] | 0.972 [0.946, 0.985] | 25.704 / 26.675 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_ct |  | 0.976 [0.970, 0.983] | 0.977 [0.959, 0.990] | 25.960 / 26.792 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | public_nonzero | yes | 0.968 [0.955, 0.979] | 0.975 [0.959, 0.988] | 25.709 / 26.839 | 0.000 | 2 / 98288 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.104 / 22.085 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_vartime |  | 1.224 [1.220, 1.240] | 1.223 [1.210, 1.251] | 25.907 / 27.059 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_ct |  | 1.236 [1.231, 1.245] | 1.230 [1.220, 1.258] | 26.064 / 27.228 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | public_nonzero | yes | 0.858 [0.851, 0.861] | 0.858 [0.849, 0.880] | 18.058 / 18.864 | 0.000 | 2 / 98272 | pass |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.251 / 4.577 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_vartime |  | 6.126 [6.062, 6.163] | 6.043 [5.931, 6.146] | 25.887 / 27.687 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_ct |  | 6.184 [6.120, 6.217] | 6.081 [5.954, 6.218] | 26.139 / 27.764 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | public_nonzero | yes | 0.356 [0.345, 0.362] | 0.378 [0.360, 0.391] | 1.510 / 1.658 | 0.000 | 1 / 81920 | pass |
| aarch64 | opt_ood | log10 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.059 / 3.175 | 0.016 | 13 / 32832 | pass |
| aarch64 | opt_ood | log10 | reuse_products |  | 0.726 [0.721, 0.731] | 0.724 [0.712, 0.740] | 2.217 / 2.311 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | indexed_products |  | 0.752 [0.748, 0.758] | 0.756 [0.746, 0.770] | 2.301 / 2.412 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | collect_products |  | 0.757 [0.753, 0.766] | 0.763 [0.745, 0.780] | 2.323 / 2.419 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | vector_products |  | 0.760 [0.756, 0.770] | 0.757 [0.747, 0.775] | 2.331 / 2.424 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | prepared_products |  | 0.751 [0.745, 0.757] | 0.752 [0.727, 0.765] | 2.294 / 2.392 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | tiled_1024 |  | 0.717 [0.714, 0.722] | 0.723 [0.709, 0.737] | 2.196 / 2.296 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_256 |  | 0.392 [0.390, 0.395] | 0.397 [0.386, 0.415] | 1.198 / 1.275 | 0.016 | 2 / 4160 | pass |
| aarch64 | opt_ood | log10 | vector_1024 |  | 0.749 [0.743, 0.757] | 0.752 [0.743, 0.768] | 2.297 / 2.406 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | vector_2048 |  | 0.749 [0.746, 0.758] | 0.755 [0.742, 0.771] | 2.296 / 2.412 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_collect |  | 0.722 [0.718, 0.729] | 0.729 [0.706, 0.737] | 2.209 / 2.307 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | tiled_grouped | yes | 0.717 [0.712, 0.721] | 0.715 [0.703, 0.739] | 2.192 / 2.299 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log16 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 66.598 / 73.753 | 1.000 | 20 / 133328 | pass |
| aarch64 | opt_ood | log16 | reuse_products |  | 0.899 [0.885, 0.912] | 0.910 [0.874, 0.985] | 59.954 / 68.363 | 1.000 | 3 / 67312 | pass |
| aarch64 | opt_ood | log16 | indexed_products |  | 0.966 [0.954, 0.980] | 0.982 [0.933, 1.003] | 64.829 / 71.250 | 1.000 | 3 / 67312 | pass |
| aarch64 | opt_ood | log16 | collect_products |  | 0.962 [0.951, 0.975] | 0.975 [0.954, 1.014] | 64.489 / 71.959 | 1.000 | 4 / 67568 | inconclusive |
| aarch64 | opt_ood | log16 | vector_products |  | 0.968 [0.956, 0.989] | 1.004 [0.956, 1.028] | 65.420 / 72.109 | 1.000 | 4 / 67568 | inconclusive |
| aarch64 | opt_ood | log16 | prepared_products |  | 0.973 [0.957, 0.985] | 0.985 [0.947, 1.007] | 64.854 / 73.060 | 1.000 | 4 / 67568 | pass |
| aarch64 | opt_ood | log16 | tiled_1024 |  | 0.705 [0.661, 0.710] | 0.701 [0.644, 0.711] | 46.601 / 48.488 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | tiled_256 |  | 0.705 [0.658, 0.710] | 0.701 [0.636, 0.707] | 46.694 / 48.296 | 1.000 | 2 / 8192 | pass |
| aarch64 | opt_ood | log16 | vector_1024 |  | 0.788 [0.739, 0.794] | 0.783 [0.718, 0.787] | 52.208 / 53.907 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | vector_2048 |  | 0.800 [0.754, 0.806] | 0.791 [0.735, 0.806] | 53.029 / 55.060 | 1.000 | 2 / 33280 | pass |
| aarch64 | opt_ood | log16 | tiled_collect |  | 0.701 [0.667, 0.709] | 0.693 [0.647, 0.704] | 46.531 / 48.478 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log16 | tiled_grouped | yes | 0.703 [0.659, 0.709] | 0.701 [0.642, 0.707] | 46.557 / 48.403 | 1.000 | 2 / 17408 | pass |
| aarch64 | opt_ood | log18 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 220.354 / 234.632 | 4.000 | 22 / 135632 | pass |
| aarch64 | opt_ood | log18 | reuse_products |  | 0.889 [0.880, 0.901] | 0.914 [0.892, 1.006] | 195.712 / 222.386 | 4.000 | 3 / 68080 | pass |
| aarch64 | opt_ood | log18 | indexed_products |  | 0.978 [0.970, 0.991] | 0.983 [0.970, 1.054] | 216.509 / 230.078 | 4.000 | 3 / 68080 | inconclusive |
| aarch64 | opt_ood | log18 | collect_products |  | 0.985 [0.975, 0.994] | 0.992 [0.981, 1.029] | 216.992 / 235.944 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | vector_products |  | 0.994 [0.987, 1.008] | 1.017 [0.989, 1.034] | 219.814 / 239.806 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | prepared_products |  | 0.987 [0.976, 0.999] | 0.995 [0.981, 1.046] | 217.499 / 235.310 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | tiled_1024 |  | 0.822 [0.805, 0.831] | 0.818 [0.795, 0.829] | 180.707 / 189.331 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | tiled_256 |  | 0.842 [0.826, 0.852] | 0.834 [0.806, 0.846] | 185.374 / 192.590 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | vector_1024 |  | 0.926 [0.907, 0.934] | 0.918 [0.884, 0.932] | 203.160 / 212.403 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | vector_2048 |  | 0.930 [0.907, 0.937] | 0.924 [0.880, 0.943] | 203.913 / 213.027 | 4.000 | 2 / 34816 | pass |
| aarch64 | opt_ood | log18 | tiled_collect |  | 0.823 [0.805, 0.832] | 0.823 [0.786, 0.832] | 181.078 / 189.833 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ood | log18 | tiled_grouped | yes | 0.827 [0.808, 0.833] | 0.818 [0.802, 0.834] | 181.410 / 189.910 | 4.000 | 2 / 20480 | pass |
| aarch64 | opt_ntt | log8_lanes1 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.557 / 2.902 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | 1.528 [1.474, 1.577] | 1.521 [1.415, 1.630] | 3.879 / 4.462 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | tiled_half |  | 1.260 [1.226, 1.284] | 1.273 [1.176, 1.355] | 3.207 / 3.781 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | half_depth |  | 1.262 [1.239, 1.289] | 1.283 [1.226, 1.348] | 3.221 / 3.787 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | incremental | yes | 0.462 [0.453, 0.464] | 0.448 [0.379, 0.468] | 1.171 / 1.242 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 45.852 / 47.346 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | depth_first |  | 0.866 [0.860, 0.869] | 0.865 [0.856, 0.879] | 39.623 / 41.242 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | tiled_half |  | 0.659 [0.656, 0.665] | 0.669 [0.656, 0.674] | 30.279 / 31.742 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | half_depth |  | 0.660 [0.656, 0.664] | 0.664 [0.654, 0.672] | 30.235 / 31.533 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | incremental | yes | 0.657 [0.652, 0.662] | 0.673 [0.657, 0.678] | 30.113 / 31.732 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 421.159 / 442.190 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | depth_first |  | 0.844 [0.836, 0.862] | 0.851 [0.832, 0.866] | 357.380 / 377.481 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | tiled_half |  | 0.774 [0.770, 0.795] | 0.781 [0.761, 0.799] | 330.250 / 347.373 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | half_depth |  | 0.778 [0.770, 0.795] | 0.781 [0.763, 0.796] | 328.034 / 346.297 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | incremental | yes | 0.778 [0.770, 0.795] | 0.789 [0.764, 0.808] | 328.414 / 349.212 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.828 / 0.878 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | chunked |  | 1.011 [1.005, 1.023] | 1.009 [0.990, 1.020] | 0.836 / 0.886 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_grid | 16 | direct_tiles |  | 0.976 [0.971, 0.990] | 0.970 [0.961, 1.039] | 0.807 / 0.889 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_grid | 16 | direct_pass | yes | 0.982 [0.974, 0.987] | 0.979 [0.967, 0.993] | 0.812 / 0.844 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 53.270 / 55.683 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | chunked |  | 1.042 [1.033, 1.056] | 1.043 [1.017, 1.059] | 55.738 / 58.348 | 1.000 | 0 / 0 | regression |
| aarch64 | opt_gf_grid | 1024 | direct_tiles |  | 0.967 [0.958, 0.979] | 0.974 [0.958, 0.985] | 51.596 / 54.484 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | direct_pass | yes | 0.965 [0.950, 0.972] | 0.965 [0.947, 0.979] | 51.386 / 53.952 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.140 / 0.145 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | fixed_schedule |  | 0.607 [0.600, 0.612] | 0.610 [0.601, 0.618] | 0.085 / 0.088 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_fused |  | 1.105 [1.097, 1.113] | 1.103 [1.084, 1.113] | 0.155 / 0.159 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_adaptive |  | 0.629 [0.624, 0.633] | 0.633 [0.621, 0.643] | 0.088 / 0.091 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | row_density |  | 0.633 [0.624, 0.636] | 0.637 [0.624, 0.646] | 0.088 / 0.092 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_masks |  | 0.997 [0.991, 1.003] | 1.004 [0.986, 1.018] | 0.140 / 0.145 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_blocks8 |  | 0.750 [0.743, 0.754] | 0.749 [0.741, 0.768] | 0.105 / 0.108 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_blocks32 |  | 0.759 [0.756, 0.766] | 0.760 [0.752, 0.776] | 0.107 / 0.110 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_classified |  | 0.671 [0.666, 0.676] | 0.671 [0.663, 0.685] | 0.094 / 0.097 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_scan |  | 0.868 [0.861, 0.875] | 0.867 [0.858, 0.878] | 0.121 / 0.126 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_prefix | yes | 0.635 [0.629, 0.639] | 0.637 [0.627, 0.654] | 0.089 / 0.093 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.227 / 8.577 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | fixed_schedule |  | 0.801 [0.789, 0.804] | 0.802 [0.788, 0.851] | 6.550 / 6.929 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_fused |  | 1.173 [1.167, 1.180] | 1.170 [1.119, 1.191] | 9.647 / 10.035 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_adaptive |  | 0.800 [0.792, 0.806] | 0.822 [0.796, 0.935] | 6.589 / 6.956 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | row_density |  | 0.669 [0.663, 0.674] | 0.672 [0.645, 0.686] | 5.508 / 5.764 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_masks |  | 1.088 [1.080, 1.095] | 1.080 [1.051, 1.097] | 8.941 / 9.258 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_blocks8 |  | 0.789 [0.780, 0.794] | 0.798 [0.777, 1.116] | 6.469 / 6.830 | 0.078 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_blocks32 |  | 0.936 [0.929, 0.942] | 0.924 [0.908, 0.949] | 7.700 / 8.072 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_classified |  | 0.700 [0.690, 0.706] | 0.703 [0.680, 0.716] | 5.751 / 6.048 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_scan |  | 0.934 [0.926, 0.939] | 0.925 [0.901, 0.941] | 7.663 / 7.974 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_prefix | yes | 0.672 [0.665, 0.676] | 0.674 [0.660, 0.700] | 5.522 / 5.811 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.064 / 0.072 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | fixed_schedule |  | 1.317 [1.210, 1.778] | 1.315 [1.189, 1.749] | 0.085 / 0.087 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_fused |  | 0.873 [0.805, 0.894] | 0.870 [0.809, 0.890] | 0.054 / 0.064 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_adaptive |  | 0.873 [0.815, 0.906] | 0.871 [0.815, 0.902] | 0.055 / 0.064 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | row_density |  | 0.997 [0.958, 1.049] | 0.984 [0.937, 1.049] | 0.066 / 0.072 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_masks |  | 1.092 [0.822, 1.461] | 1.062 [0.830, 1.392] | 0.067 / 0.078 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_blocks8 |  | 0.967 [0.914, 1.031] | 0.962 [0.907, 0.996] | 0.061 / 0.072 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_blocks32 |  | 0.905 [0.855, 0.947] | 0.905 [0.861, 0.944] | 0.057 / 0.067 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_classified |  | 1.032 [1.008, 1.088] | 1.024 [1.002, 1.106] | 0.069 / 0.074 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_scan |  | 0.877 [0.817, 0.933] | 0.877 [0.829, 0.924] | 0.055 / 0.065 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_prefix | yes | 0.894 [0.834, 0.917] | 0.902 [0.838, 0.934] | 0.057 / 0.066 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.566 / 3.898 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | fixed_schedule |  | 1.835 [1.747, 1.896] | 1.810 [1.657, 1.905] | 6.542 / 6.975 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_fused |  | 0.845 [0.827, 0.852] | 0.853 [0.805, 0.891] | 3.011 / 3.218 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_adaptive |  | 0.842 [0.831, 0.852] | 0.850 [0.815, 0.882] | 3.009 / 3.212 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | row_density |  | 1.460 [1.440, 1.481] | 1.425 [1.381, 1.447] | 5.202 / 5.574 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_masks |  | 1.057 [1.002, 1.103] | 1.046 [0.984, 1.073] | 3.774 / 4.241 | 0.078 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_blocks8 |  | 0.989 [0.956, 1.001] | 0.978 [0.925, 1.002] | 3.493 / 3.712 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_blocks32 |  | 0.880 [0.862, 0.886] | 0.890 [0.852, 0.912] | 3.122 / 3.398 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_classified |  | 1.470 [1.441, 1.483] | 1.431 [1.372, 1.465] | 5.220 / 5.587 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_scan |  | 0.845 [0.829, 0.855] | 0.834 [0.817, 0.867] | 3.010 / 3.231 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_prefix | yes | 0.847 [0.826, 0.856] | 0.843 [0.817, 0.860] | 3.011 / 3.244 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.020 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | fixed_schedule |  | 4.233 [4.212, 4.267] | 4.188 [4.107, 4.258] | 0.085 / 0.087 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_fused |  | 0.930 [0.924, 0.936] | 0.924 [0.904, 0.942] | 0.019 / 0.019 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_adaptive |  | 0.961 [0.956, 0.972] | 0.963 [0.934, 0.990] | 0.019 / 0.020 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | row_density |  | 0.431 [0.430, 0.437] | 0.439 [0.427, 0.448] | 0.009 / 0.009 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_masks |  | 0.875 [0.868, 0.937] | 0.875 [0.858, 0.925] | 0.018 / 0.019 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_blocks8 |  | 1.267 [1.257, 1.274] | 1.256 [1.227, 1.288] | 0.025 / 0.026 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_blocks32 |  | 1.075 [1.067, 1.081] | 1.073 [1.047, 1.087] | 0.021 / 0.022 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_classified |  | 0.433 [0.430, 0.436] | 0.441 [0.428, 0.448] | 0.009 / 0.009 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_scan |  | 0.472 [0.468, 0.476] | 0.477 [0.465, 0.483] | 0.009 / 0.010 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_prefix | yes | 0.472 [0.470, 0.479] | 0.480 [0.468, 0.490] | 0.009 / 0.010 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.100 / 1.157 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | fixed_schedule |  | 5.948 [5.892, 6.006] | 5.893 [5.723, 5.969] | 6.534 / 6.724 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_fused |  | 0.891 [0.882, 0.896] | 0.888 [0.861, 0.905] | 0.977 / 1.012 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_adaptive |  | 0.891 [0.880, 0.900] | 0.885 [0.872, 0.901] | 0.980 / 1.023 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | row_density |  | 0.375 [0.369, 0.378] | 0.380 [0.371, 0.389] | 0.410 / 0.440 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_masks |  | 0.844 [0.837, 0.850] | 0.847 [0.828, 0.853] | 0.925 / 0.971 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_blocks8 |  | 1.290 [1.279, 1.304] | 1.287 [1.243, 1.311] | 1.418 / 1.477 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_blocks32 |  | 0.994 [0.980, 0.999] | 0.985 [0.962, 1.013] | 1.089 / 1.135 | 0.078 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_classified |  | 0.374 [0.371, 0.377] | 0.384 [0.371, 0.391] | 0.410 / 0.438 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_scan |  | 0.406 [0.403, 0.412] | 0.416 [0.400, 0.424] | 0.448 / 0.478 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_prefix | yes | 0.406 [0.402, 0.411] | 0.419 [0.403, 0.427] | 0.446 / 0.478 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.089 / 0.102 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | fixed_schedule |  | 0.968 [0.864, 0.989] | 0.961 [0.869, 1.005] | 0.086 / 0.090 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_fused |  | 0.932 [0.922, 0.985] | 0.944 [0.905, 0.983] | 0.085 / 0.094 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_adaptive |  | 0.995 [0.895, 1.036] | 1.011 [0.885, 1.037] | 0.088 / 0.092 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | row_density |  | 0.855 [0.797, 0.902] | 0.876 [0.802, 0.903] | 0.078 / 0.091 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_masks |  | 1.104 [0.821, 1.260] | 1.113 [0.809, 1.253] | 0.098 / 0.111 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_blocks8 |  | 1.017 [0.993, 1.058] | 1.046 [0.988, 1.073] | 0.093 / 0.101 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_blocks32 |  | 0.952 [0.946, 1.006] | 0.967 [0.914, 1.006] | 0.087 / 0.098 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_classified |  | 0.859 [0.820, 0.900] | 0.860 [0.830, 0.912] | 0.077 / 0.092 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_scan |  | 0.951 [0.935, 1.005] | 0.938 [0.921, 1.005] | 0.087 / 0.095 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_prefix | yes | 0.731 [0.709, 0.764] | 0.750 [0.720, 0.773] | 0.066 / 0.075 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.600 / 3.866 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | fixed_schedule |  | 1.819 [1.782, 1.886] | 1.780 [1.674, 1.876] | 6.580 / 6.993 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_fused |  | 0.851 [0.836, 0.861] | 0.859 [0.787, 0.881] | 3.052 / 3.320 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_adaptive |  | 1.825 [1.791, 1.886] | 1.752 [1.700, 1.839] | 6.598 / 6.841 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | row_density |  | 1.458 [1.446, 1.494] | 1.446 [1.345, 1.500] | 5.299 / 5.620 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_masks |  | 1.093 [1.060, 1.105] | 1.063 [0.967, 1.097] | 3.900 / 4.142 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_blocks8 |  | 0.986 [0.976, 1.001] | 0.980 [0.637, 1.003] | 3.553 / 3.794 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_blocks32 |  | 0.883 [0.872, 0.899] | 0.884 [0.852, 0.925] | 3.189 / 3.443 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_classified |  | 1.462 [1.448, 1.489] | 1.437 [1.367, 1.486] | 5.298 / 5.585 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_scan |  | 0.847 [0.838, 0.869] | 0.859 [0.786, 0.891] | 3.073 / 3.387 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_prefix | yes | 0.850 [0.837, 0.858] | 0.836 [0.613, 0.866] | 3.036 / 3.239 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.106 / 0.110 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | fixed_schedule |  | 0.804 [0.795, 0.811] | 0.814 [0.787, 0.827] | 0.085 / 0.089 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_fused |  | 1.113 [1.105, 1.124] | 1.111 [1.097, 1.138] | 0.118 / 0.123 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_adaptive |  | 1.114 [1.104, 1.126] | 1.127 [1.105, 1.163] | 0.118 / 0.123 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | row_density |  | 0.650 [0.641, 0.657] | 0.658 [0.644, 0.675] | 0.069 / 0.072 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_masks |  | 0.986 [0.980, 0.995] | 0.987 [0.966, 1.008] | 0.105 / 0.109 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_blocks8 |  | 0.910 [0.904, 0.920] | 0.930 [0.897, 0.946] | 0.097 / 0.101 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_blocks32 |  | 1.144 [1.126, 1.159] | 1.154 [1.127, 1.183] | 0.121 / 0.128 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_classified |  | 0.687 [0.679, 0.692] | 0.695 [0.677, 0.712] | 0.073 / 0.076 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_scan |  | 0.905 [0.893, 0.912] | 0.889 [0.880, 0.924] | 0.095 / 0.099 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_prefix | yes | 0.651 [0.645, 0.659] | 0.662 [0.641, 0.671] | 0.069 / 0.073 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.192 / 8.441 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | fixed_schedule |  | 0.804 [0.795, 0.808] | 0.811 [0.800, 0.820] | 6.574 / 6.853 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_fused |  | 1.170 [1.162, 1.176] | 1.174 [1.158, 1.193] | 9.570 / 9.924 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_adaptive |  | 1.163 [1.157, 1.171] | 1.160 [1.152, 1.178] | 9.538 / 9.821 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | row_density |  | 0.671 [0.665, 0.677] | 0.672 [0.665, 0.682] | 5.496 / 5.677 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_masks |  | 1.089 [1.083, 1.095] | 1.093 [1.079, 1.105] | 8.905 / 9.246 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_blocks8 |  | 0.791 [0.784, 0.795] | 0.795 [0.786, 0.807] | 6.471 / 6.694 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_blocks32 |  | 0.946 [0.936, 0.951] | 0.948 [0.938, 0.968] | 7.712 / 8.010 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_classified |  | 0.676 [0.669, 0.680] | 0.685 [0.676, 0.697] | 5.527 / 5.820 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_scan |  | 0.936 [0.927, 0.941] | 0.933 [0.924, 0.954] | 7.651 / 7.932 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_prefix | yes | 0.670 [0.665, 0.677] | 0.675 [0.667, 0.691] | 5.495 / 5.718 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.078 / 0.080 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | fixed_schedule |  | 1.096 [1.086, 1.101] | 1.096 [1.082, 1.116] | 0.086 / 0.089 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_fused |  | 1.021 [1.016, 1.029] | 1.021 [1.011, 1.031] | 0.080 / 0.082 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_adaptive |  | 1.026 [1.018, 1.030] | 1.023 [1.014, 1.039] | 0.080 / 0.083 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | row_density |  | 0.624 [0.614, 0.627] | 0.628 [0.620, 0.638] | 0.049 / 0.051 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_masks |  | 0.896 [0.887, 0.900] | 0.893 [0.884, 0.907] | 0.070 / 0.072 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_blocks8 |  | 1.117 [1.111, 1.123] | 1.116 [1.107, 1.124] | 0.088 / 0.090 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_blocks32 |  | 1.069 [1.062, 1.076] | 1.068 [1.057, 1.081] | 0.084 / 0.086 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_classified |  | 0.644 [0.639, 0.647] | 0.649 [0.638, 0.656] | 0.050 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_scan |  | 0.866 [0.855, 0.872] | 0.869 [0.860, 0.880] | 0.068 / 0.070 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_prefix | yes | 0.628 [0.623, 0.632] | 0.636 [0.622, 0.649] | 0.049 / 0.051 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.648 / 4.812 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | fixed_schedule |  | 1.418 [1.408, 1.427] | 1.402 [1.382, 1.425] | 6.592 / 6.739 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_fused |  | 1.043 [1.038, 1.055] | 1.040 [1.021, 1.049] | 4.866 / 4.989 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_adaptive |  | 1.045 [1.040, 1.056] | 1.047 [1.020, 1.061] | 4.872 / 5.052 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | row_density |  | 0.640 [0.635, 0.646] | 0.638 [0.630, 0.656] | 2.977 / 3.081 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_masks |  | 0.965 [0.959, 0.974] | 0.967 [0.945, 0.979] | 4.494 / 4.639 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_blocks8 |  | 1.124 [1.116, 1.133] | 1.108 [1.090, 1.130] | 5.222 / 5.372 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_blocks32 |  | 1.065 [1.057, 1.072] | 1.051 [1.040, 1.071] | 4.946 / 5.094 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_classified |  | 0.666 [0.661, 0.674] | 0.667 [0.655, 0.679] | 3.104 / 3.219 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_scan |  | 0.914 [0.906, 0.922] | 0.908 [0.899, 0.920] | 4.248 / 4.366 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_prefix | yes | 0.640 [0.633, 0.646] | 0.644 [0.634, 0.656] | 2.983 / 3.100 | 0.078 | 0 / 0 | pass |
