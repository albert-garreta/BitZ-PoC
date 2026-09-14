# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_integer_mac | l1_signed16_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_signed16_n16 | products2 |  | 1.263 [1.255, 1.306] | 1.257 [1.231, 1.335] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l1_signed16_n16 | products4 |  | 1.029 [1.002, 1.174] | 1.016 [0.988, 1.178] | 0.006 / 0.007 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n16 | deferred_columns |  | 1.011 [1.003, 1.028] | 0.998 [0.976, 1.082] | 0.006 / 0.006 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n16 | incumbent | yes | 1.054 [1.047, 1.064] | 1.052 [1.009, 1.067] | 0.006 / 0.006 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.332 / 0.347 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | products2 |  | 1.009 [1.004, 1.016] | 1.008 [0.999, 1.027] | 0.335 / 0.352 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | products4 |  | 1.000 [0.994, 1.005] | 0.999 [0.985, 1.019] | 0.332 / 0.346 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | deferred_columns |  | 0.999 [0.991, 1.010] | 1.006 [0.993, 1.021] | 0.332 / 0.349 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | incumbent | yes | 1.004 [0.993, 1.009] | 1.008 [0.994, 1.016] | 0.333 / 0.349 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 20.830 / 21.718 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | products2 |  | 0.997 [0.993, 1.010] | 0.997 [0.988, 1.006] | 20.897 / 21.630 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | products4 |  | 1.001 [0.995, 1.005] | 0.994 [0.985, 1.006] | 20.835 / 21.628 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | deferred_columns |  | 1.000 [0.995, 1.006] | 0.999 [0.985, 1.006] | 20.789 / 21.678 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | incumbent | yes | 1.001 [0.996, 1.005] | 1.001 [0.990, 1.008] | 20.860 / 21.680 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n16 | products2 |  | 1.253 [1.241, 1.294] | 1.258 [1.238, 1.292] | 0.007 / 0.007 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l1_full_n16 | products4 |  | 1.013 [1.007, 1.026] | 1.018 [1.006, 1.033] | 0.006 / 0.006 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n16 | deferred_columns |  | 1.000 [0.992, 1.028] | 1.012 [0.986, 1.027] | 0.006 / 0.006 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n16 | incumbent | yes | 1.051 [1.043, 1.055] | 1.044 [1.038, 1.064] | 0.006 / 0.006 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l1_full_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.329 / 0.343 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n1024 | products2 |  | 1.007 [1.002, 1.013] | 1.005 [0.997, 1.016] | 0.332 / 0.347 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n1024 | products4 |  | 0.996 [0.993, 1.007] | 0.996 [0.986, 1.016] | 0.329 / 0.344 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n1024 | deferred_columns |  | 0.999 [0.992, 1.003] | 0.997 [0.988, 1.009] | 0.329 / 0.342 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n1024 | incumbent | yes | 1.000 [0.992, 1.008] | 1.000 [0.988, 1.013] | 0.329 / 0.342 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 20.794 / 21.594 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n65536 | products2 |  | 1.004 [0.999, 1.009] | 1.003 [0.996, 1.019] | 20.832 / 21.703 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n65536 | products4 |  | 1.001 [0.998, 1.009] | 1.000 [0.992, 1.024] | 20.839 / 21.681 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n65536 | deferred_columns |  | 1.003 [0.998, 1.009] | 1.008 [0.990, 1.021] | 20.849 / 21.604 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n65536 | incumbent | yes | 1.002 [0.996, 1.009] | 1.003 [0.993, 1.020] | 20.788 / 21.674 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l2_signed16_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n16 | products2 |  | 0.899 [0.893, 0.915] | 0.909 [0.896, 0.926] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n16 | products4 |  | 0.938 [0.930, 0.946] | 0.943 [0.932, 0.970] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n16 | deferred_columns |  | 1.584 [1.578, 1.628] | 1.572 [1.554, 1.626] | 0.020 / 0.021 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_signed16_n16 | incumbent | yes | 0.848 [0.843, 0.853] | 0.854 [0.846, 0.872] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.691 / 0.757 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | products2 |  | 0.955 [0.945, 0.961] | 0.949 [0.795, 0.962] | 0.660 / 0.696 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | products4 |  | 0.957 [0.950, 0.961] | 0.956 [0.799, 0.971] | 0.661 / 0.700 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | deferred_columns |  | 1.683 [1.670, 1.692] | 1.675 [1.621, 1.792] | 1.165 / 1.242 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | incumbent | yes | 0.952 [0.945, 0.956] | 0.940 [0.820, 0.959] | 0.657 / 0.694 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 43.792 / 45.524 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | products2 |  | 0.950 [0.945, 0.954] | 0.954 [0.929, 0.967] | 41.605 / 43.141 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | products4 |  | 0.952 [0.949, 0.956] | 0.952 [0.938, 0.967] | 41.719 / 43.371 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | deferred_columns |  | 1.661 [1.657, 1.677] | 1.664 [1.613, 1.685] | 73.142 / 74.870 | 2.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | incumbent | yes | 0.952 [0.948, 0.958] | 0.949 [0.926, 0.965] | 41.710 / 43.078 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n16 | products2 |  | 0.902 [0.894, 0.915] | 0.903 [0.882, 0.918] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n16 | products4 |  | 0.943 [0.936, 0.953] | 0.940 [0.931, 0.966] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n16 | deferred_columns |  | 1.587 [1.582, 1.642] | 1.571 [1.562, 1.615] | 0.020 / 0.021 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_full_n16 | incumbent | yes | 0.850 [0.846, 0.855] | 0.858 [0.847, 0.866] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.691 / 0.714 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n1024 | products2 |  | 0.956 [0.952, 0.961] | 0.967 [0.954, 0.978] | 0.661 / 0.689 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n1024 | products4 |  | 0.959 [0.951, 0.965] | 0.951 [0.941, 0.965] | 0.661 / 0.681 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n1024 | deferred_columns |  | 1.687 [1.679, 1.694] | 1.693 [1.670, 1.719] | 1.166 / 1.212 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_full_n1024 | incumbent | yes | 0.954 [0.948, 0.958] | 0.952 [0.942, 0.966] | 0.658 / 0.682 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 43.992 / 46.003 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n65536 | products2 |  | 0.952 [0.948, 0.958] | 0.954 [0.934, 0.963] | 41.954 / 43.445 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n65536 | products4 |  | 0.950 [0.944, 0.957] | 0.948 [0.942, 0.959] | 41.840 / 43.621 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n65536 | deferred_columns |  | 1.665 [1.655, 1.681] | 1.657 [1.623, 1.667] | 73.370 / 75.968 | 2.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_full_n65536 | incumbent | yes | 0.949 [0.943, 0.957] | 0.945 [0.937, 0.960] | 41.800 / 43.492 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.341 / 0.370 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n16 | products2 |  | 1.025 [1.015, 1.033] | 1.017 [0.820, 1.049] | 0.350 / 0.371 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_signed16_n16 | products4 |  | 1.049 [1.040, 1.064] | 1.057 [0.964, 1.077] | 0.360 / 0.384 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_signed16_n16 | deferred_columns |  | 2.465 [2.451, 2.502] | 2.464 [2.020, 2.527] | 0.843 / 0.917 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_signed16_n16 | incumbent | yes | 0.808 [0.799, 0.818] | 0.804 [0.787, 0.904] | 0.276 / 0.292 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.726 / 22.543 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | products2 |  | 1.009 [1.003, 1.013] | 1.007 [0.995, 1.020] | 21.876 / 22.771 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | products4 |  | 1.009 [1.003, 1.014] | 1.000 [0.993, 1.026] | 21.907 / 22.837 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | deferred_columns |  | 2.497 [2.484, 2.508] | 2.470 [2.448, 2.525] | 54.142 / 56.120 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | incumbent | yes | 0.802 [0.798, 0.809] | 0.802 [0.794, 0.813] | 17.484 / 18.111 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1403.239 / 1485.061 | 9.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | products2 |  | 1.012 [1.004, 1.019] | 1.010 [0.997, 1.022] | 1420.005 / 1500.355 | 9.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | products4 |  | 1.012 [1.001, 1.017] | 1.009 [0.994, 1.022] | 1417.860 / 1497.238 | 9.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | deferred_columns |  | 2.510 [2.497, 2.526] | 2.477 [2.448, 2.509] | 3514.396 / 3738.978 | 9.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | incumbent | yes | 0.810 [0.801, 0.820] | 0.815 [0.803, 0.829] | 1136.521 / 1209.565 | 9.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.341 / 0.370 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n16 | products2 |  | 1.024 [1.015, 1.033] | 1.016 [0.947, 1.035] | 0.348 / 0.372 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_full_n16 | products4 |  | 1.067 [1.052, 1.077] | 1.070 [1.017, 1.085] | 0.361 / 0.390 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_full_n16 | deferred_columns |  | 2.494 [2.470, 2.506] | 2.427 [2.274, 2.501] | 0.843 / 0.916 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_full_n16 | incumbent | yes | 0.808 [0.802, 0.818] | 0.814 [0.788, 0.837] | 0.275 / 0.298 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.846 / 23.658 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n1024 | products2 |  | 1.015 [1.001, 1.019] | 1.011 [0.995, 1.030] | 22.029 / 23.676 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_full_n1024 | products4 |  | 1.007 [1.000, 1.016] | 1.007 [0.988, 1.028] | 22.048 / 23.778 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_full_n1024 | deferred_columns |  | 2.489 [2.471, 2.514] | 2.464 [2.437, 2.499] | 54.410 / 57.870 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_full_n1024 | incumbent | yes | 0.811 [0.797, 0.814] | 0.804 [0.789, 0.817] | 17.645 / 18.668 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1413.261 / 1500.337 | 9.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n65536 | products2 |  | 1.007 [1.000, 1.014] | 1.003 [0.990, 1.019] | 1421.807 / 1505.689 | 9.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_full_n65536 | products4 |  | 1.012 [1.002, 1.017] | 1.010 [0.995, 1.025] | 1423.182 / 1508.455 | 9.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_full_n65536 | deferred_columns |  | 2.505 [2.492, 2.518] | 2.482 [2.452, 2.515] | 3526.609 / 3746.507 | 9.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_full_n65536 | incumbent | yes | 0.807 [0.801, 0.816] | 0.804 [0.796, 0.816] | 1140.136 / 1216.014 | 9.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.054 / 0.057 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n16 | products2 |  | 1.079 [1.067, 1.091] | 1.086 [1.058, 1.101] | 0.058 / 0.063 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n16 | products4 |  | 1.091 [1.084, 1.105] | 1.086 [1.066, 1.112] | 0.059 / 0.062 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n16 | deferred_columns |  | 1.217 [1.207, 1.233] | 1.222 [1.198, 1.240] | 0.066 / 0.070 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n16 | incumbent |  | 1.065 [1.055, 1.078] | 1.062 [1.048, 1.080] | 0.058 / 0.061 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n16 | native128x2 | yes | 0.901 [0.892, 0.912] | 0.906 [0.890, 0.920] | 0.049 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n16 | native128x2_acc2 |  | 0.896 [0.886, 0.905] | 0.898 [0.885, 0.915] | 0.048 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.160 / 3.340 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | products2 |  | 1.073 [1.061, 1.082] | 1.078 [1.051, 1.091] | 3.385 / 3.556 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | products4 |  | 1.116 [1.106, 1.130] | 1.123 [1.105, 1.131] | 3.532 / 3.734 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | deferred_columns |  | 1.283 [1.274, 1.302] | 1.299 [1.264, 1.312] | 4.072 / 4.264 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | incumbent |  | 1.329 [1.316, 1.339] | 1.320 [1.295, 1.330] | 4.199 / 4.386 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | native128x2 | yes | 0.909 [0.901, 0.917] | 0.928 [0.903, 0.939] | 2.881 / 3.076 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | native128x2_acc2 |  | 0.932 [0.921, 0.940] | 0.933 [0.915, 0.958] | 2.949 / 3.101 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 206.005 / 217.250 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | products2 |  | 1.087 [1.074, 1.094] | 1.086 [1.061, 1.109] | 222.645 / 239.544 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | products4 |  | 1.103 [1.086, 1.108] | 1.103 [1.070, 1.132] | 226.105 / 242.081 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | deferred_columns |  | 1.298 [1.281, 1.305] | 1.290 [1.265, 1.321] | 266.612 / 282.376 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | incumbent |  | 1.324 [1.302, 1.331] | 1.313 [1.277, 1.328] | 271.410 / 283.763 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | native128x2 | yes | 0.924 [0.914, 0.934] | 0.928 [0.907, 0.942] | 190.243 / 202.521 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | native128x2_acc2 |  | 0.916 [0.908, 0.924] | 0.924 [0.900, 0.941] | 189.176 / 202.223 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.054 / 0.058 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n16 | products2 |  | 1.082 [1.069, 1.094] | 1.080 [0.963, 1.097] | 0.058 / 0.062 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | products4 |  | 1.097 [1.084, 1.109] | 1.089 [0.587, 1.114] | 0.059 / 0.063 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | deferred_columns |  | 1.223 [1.206, 1.235] | 1.217 [1.179, 1.245] | 0.066 / 0.070 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | incumbent |  | 1.064 [1.058, 1.077] | 1.074 [0.991, 1.093] | 0.058 / 0.061 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | native128x2 | yes | 0.903 [0.894, 0.914] | 0.898 [0.834, 0.911] | 0.049 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n16 | native128x2_acc2 |  | 0.898 [0.889, 0.909] | 0.903 [0.766, 0.926] | 0.048 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.157 / 3.379 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n1024 | products2 |  | 1.070 [1.060, 1.081] | 1.066 [1.047, 1.076] | 3.382 / 3.598 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n1024 | products4 |  | 1.118 [1.102, 1.122] | 1.110 [1.094, 1.127] | 3.516 / 3.764 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n1024 | deferred_columns |  | 1.289 [1.279, 1.299] | 1.278 [1.256, 1.298] | 4.066 / 4.309 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n1024 | incumbent |  | 1.325 [1.315, 1.340] | 1.326 [1.301, 1.344] | 4.201 / 4.486 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n1024 | native128x2 | yes | 0.911 [0.904, 0.921] | 0.915 [0.901, 0.933] | 2.881 / 3.100 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n1024 | native128x2_acc2 |  | 0.941 [0.928, 0.948] | 0.937 [0.922, 0.949] | 2.955 / 3.153 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 205.615 / 215.702 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n65536 | products2 |  | 1.079 [1.065, 1.085] | 1.069 [1.052, 1.088] | 221.391 / 229.910 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n65536 | products4 |  | 1.096 [1.089, 1.108] | 1.091 [1.078, 1.108] | 225.841 / 235.284 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n65536 | deferred_columns |  | 1.293 [1.282, 1.302] | 1.279 [1.261, 1.299] | 266.270 / 274.731 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n65536 | incumbent |  | 1.308 [1.299, 1.325] | 1.298 [1.278, 1.310] | 269.677 / 277.611 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n65536 | native128x2 | yes | 0.921 [0.910, 0.928] | 0.922 [0.909, 0.935] | 189.293 / 198.950 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n65536 | native128x2_acc2 |  | 0.916 [0.907, 0.920] | 0.918 [0.901, 0.931] | 188.301 / 196.753 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.853 / 3.143 | 0.000 | 9 / 136 | pass |
| aarch64 | opt_projection | q100_l2_n16 | public_divisor |  | 0.336 [0.319, 0.341] | 0.340 [0.332, 0.356] | 0.972 / 1.055 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n16 | horner_prepared | yes | 0.042 [0.041, 0.044] | 0.042 [0.041, 0.052] | 0.122 / 0.134 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n16 | horner_one_shot | yes | 0.097 [0.095, 0.100] | 0.097 [0.095, 0.115] | 0.278 / 0.303 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 566.664 / 605.141 | 0.031 | 519 / 8296 | pass |
| aarch64 | opt_projection | q100_l2_n1024 | public_divisor |  | 0.115 [0.113, 0.115] | 0.114 [0.112, 0.118] | 64.458 / 69.613 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n1024 | horner_prepared | yes | 0.014 [0.013, 0.014] | 0.014 [0.013, 0.014] | 7.622 / 8.221 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n1024 | horner_one_shot | yes | 0.014 [0.014, 0.014] | 0.014 [0.014, 0.014] | 7.825 / 8.429 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.322 / 0.400 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n16 | public_divisor |  | 2.388 [1.925, 2.838] | 2.324 [1.899, 2.736] | 0.761 / 0.809 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_projection | q128_l2_n16 | horner_prepared | yes | 0.381 [0.308, 0.458] | 0.380 [0.313, 0.455] | 0.123 / 0.131 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n16 | horner_one_shot | yes | 0.854 [0.683, 1.013] | 0.846 [0.683, 0.989] | 0.271 / 0.289 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_projection | q128_l2_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 22.794 / 24.713 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n1024 | public_divisor |  | 2.115 [2.086, 2.132] | 2.056 [2.019, 2.106] | 48.031 / 51.219 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_projection | q128_l2_n1024 | horner_prepared | yes | 0.334 [0.330, 0.339] | 0.340 [0.324, 0.364] | 7.621 / 8.349 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n1024 | horner_one_shot | yes | 0.341 [0.337, 0.347] | 0.340 [0.334, 0.353] | 7.803 / 8.395 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 5.667 / 6.633 | 0.001 | 11 / 328 | pass |
| aarch64 | opt_projection | q100_l4_n16 | public_divisor |  | 0.239 [0.233, 0.244] | 0.229 [0.202, 0.241] | 1.357 / 1.468 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n16 | horner_prepared | yes | 0.082 [0.079, 0.083] | 0.075 [0.069, 0.084] | 0.463 / 0.505 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n16 | horner_one_shot | yes | 0.112 [0.110, 0.114] | 0.102 [0.096, 0.116] | 0.637 / 0.699 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1379.516 / 1488.256 | 0.047 | 543 / 17352 | pass |
| aarch64 | opt_projection | q100_l4_n1024 | public_divisor |  | 0.061 [0.060, 0.061] | 0.065 [0.061, 0.068] | 83.562 / 95.755 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n1024 | horner_prepared | yes | 0.021 [0.021, 0.022] | 0.022 [0.021, 0.023] | 29.562 / 32.875 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n1024 | horner_one_shot | yes | 0.022 [0.021, 0.022] | 0.022 [0.022, 0.023] | 29.766 / 32.703 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.016 / 1.190 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n16 | public_divisor |  | 1.213 [1.081, 1.428] | 1.162 [1.047, 1.371] | 1.252 / 1.347 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_projection | q128_l4_n16 | horner_prepared | yes | 0.457 [0.406, 0.534] | 0.445 [0.408, 0.497] | 0.471 / 0.503 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n16 | horner_one_shot | yes | 0.615 [0.545, 0.714] | 0.610 [0.544, 0.679] | 0.633 / 0.677 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 84.139 / 90.750 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n1024 | public_divisor |  | 0.956 [0.928, 0.966] | 0.932 [0.918, 0.952] | 79.326 / 85.580 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n1024 | horner_prepared | yes | 0.356 [0.351, 0.362] | 0.356 [0.347, 0.366] | 29.695 / 32.321 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n1024 | horner_one_shot | yes | 0.359 [0.353, 0.366] | 0.363 [0.351, 0.372] | 30.022 / 32.729 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 28.483 / 36.014 | 0.001 | 12 / 800 | pass |
| aarch64 | opt_projection | q100_l9_n16 | public_divisor |  | 0.098 [0.091, 0.103] | 0.091 [0.083, 0.101] | 2.845 / 3.123 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n16 | horner_prepared | yes | 0.051 [0.048, 0.054] | 0.045 [0.043, 0.050] | 1.480 / 1.605 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n16 | horner_one_shot | yes | 0.059 [0.056, 0.064] | 0.054 [0.049, 0.061] | 1.727 / 1.853 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3366.874 / 3514.552 | 0.086 | 531 / 38168 | pass |
| aarch64 | opt_projection | q100_l9_n1024 | public_divisor |  | 0.053 [0.053, 0.054] | 0.054 [0.052, 0.056] | 177.792 / 190.040 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n1024 | horner_prepared | yes | 0.028 [0.028, 0.028] | 0.028 [0.028, 0.029] | 94.791 / 100.468 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n1024 | horner_one_shot | yes | 0.028 [0.028, 0.029] | 0.029 [0.028, 0.029] | 95.105 / 101.127 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.922 / 3.076 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n16 | public_divisor |  | 0.945 [0.919, 1.029] | 0.939 [0.911, 1.016] | 2.751 / 2.864 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_projection | q128_l9_n16 | horner_prepared | yes | 0.512 [0.499, 0.562] | 0.522 [0.501, 0.570] | 1.495 / 1.593 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n16 | horner_one_shot | yes | 0.600 [0.581, 0.644] | 0.593 [0.578, 0.649] | 1.733 / 1.827 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 235.411 / 244.289 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n1024 | public_divisor |  | 0.731 [0.723, 0.739] | 0.739 [0.727, 0.754] | 171.393 / 183.511 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n1024 | horner_prepared | yes | 0.405 [0.403, 0.410] | 0.410 [0.402, 0.423] | 95.406 / 101.146 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n1024 | horner_one_shot | yes | 0.406 [0.403, 0.409] | 0.414 [0.403, 0.423] | 95.585 / 102.368 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q100_l2 | horner |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.156 / 0.168 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q128_l2 | horner |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.147 / 0.157 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q100_l4 | horner |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.179 / 0.316 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q128_l4 | horner |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.169 / 0.181 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q100_l9 | horner |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.228 / 0.234 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q128_l9 | horner |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.220 / 0.229 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n16 | generic |  | 2.179 [2.168, 2.196] | 2.142 [2.115, 2.190] | 0.816 / 0.850 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l2_n16 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.375 / 0.397 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n16 | prepared_reciprocal | yes | 0.264 [0.261, 0.266] | 0.270 [0.261, 0.279] | 0.098 / 0.106 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n16 | prepare_batch |  | 0.333 [0.330, 0.335] | 0.329 [0.324, 0.348] | 0.124 / 0.132 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n1024 | generic |  | 2.206 [2.189, 2.215] | 2.178 [2.152, 2.210] | 52.047 / 55.390 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l2_n1024 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 23.692 / 25.270 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n1024 | prepared_reciprocal | yes | 0.265 [0.264, 0.267] | 0.269 [0.263, 0.278] | 6.288 / 6.843 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n1024 | prepare_batch |  | 0.276 [0.274, 0.278] | 0.282 [0.276, 0.291] | 6.552 / 7.070 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n16 | generic |  | 2.394 [2.385, 2.414] | 2.381 [2.352, 2.433] | 1.512 / 1.598 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l4_n16 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.631 / 0.669 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n16 | prepared_reciprocal | yes | 0.393 [0.389, 0.396] | 0.402 [0.389, 0.408] | 0.248 / 0.264 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n16 | prepare_batch |  | 0.429 [0.426, 0.434] | 0.437 [0.428, 0.445] | 0.271 / 0.292 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n1024 | generic |  | 2.399 [2.382, 2.423] | 2.368 [2.341, 2.419] | 96.335 / 100.440 | 0.094 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l4_n1024 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 40.265 / 42.167 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n1024 | prepared_reciprocal | yes | 0.393 [0.391, 0.400] | 0.401 [0.392, 0.409] | 15.830 / 16.823 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n1024 | prepare_batch |  | 0.402 [0.397, 0.405] | 0.407 [0.399, 0.418] | 16.080 / 17.122 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n16 | generic |  | 3.360 [3.346, 3.389] | 3.379 [2.768, 3.476] | 4.321 / 4.646 | 0.003 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l9_n16 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.283 / 1.422 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n16 | prepared_reciprocal | yes | 0.544 [0.539, 0.551] | 0.559 [0.539, 0.756] | 0.700 / 0.855 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n16 | prepare_batch |  | 0.568 [0.565, 0.575] | 0.583 [0.529, 0.602] | 0.734 / 0.809 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n1024 | generic |  | 3.383 [3.359, 3.414] | 3.351 [3.272, 3.393] | 274.937 / 294.043 | 0.211 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l9_n1024 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 81.687 / 87.925 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n1024 | prepared_reciprocal | yes | 0.546 [0.542, 0.555] | 0.546 [0.511, 0.556] | 44.497 / 47.946 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n1024 | prepare_batch |  | 0.550 [0.545, 0.554] | 0.552 [0.537, 0.570] | 44.744 / 49.464 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n16 | generic |  | 5.608 [5.568, 5.624] | 5.509 [5.415, 5.620] | 26.874 / 29.010 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l32_n16 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.838 / 5.203 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n16 | prepared_reciprocal | yes | 0.833 [0.828, 0.838] | 0.837 [0.782, 0.842] | 4.035 / 4.264 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n16 | prepare_batch |  | 0.839 [0.832, 0.842] | 0.837 [0.821, 0.844] | 4.032 / 4.417 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n1024 | generic |  | 5.592 [5.555, 5.646] | 5.530 [5.440, 5.653] | 1722.661 / 1839.827 | 0.750 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l32_n1024 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 308.246 / 329.801 | 0.750 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n1024 | prepared_reciprocal | yes | 0.827 [0.823, 0.839] | 0.829 [0.819, 0.843] | 256.469 / 277.606 | 0.750 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n1024 | prepare_batch |  | 0.834 [0.821, 0.842] | 0.830 [0.817, 0.850] | 257.527 / 276.023 | 0.750 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n16 | generic |  | 9.310 [9.248, 9.373] | 9.069 [8.838, 9.231] | 87.189 / 94.081 | 0.023 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l64_n16 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 9.384 / 10.278 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n16 | prepared_reciprocal | yes | 0.862 [0.854, 0.867] | 0.859 [0.839, 0.874] | 8.091 / 8.796 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n16 | prepare_batch |  | 0.864 [0.853, 0.874] | 0.869 [0.847, 0.890] | 8.129 / 8.999 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n1024 | generic |  | 9.276 [9.186, 9.308] | 9.061 [8.878, 9.208] | 5573.711 / 6017.822 | 1.500 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l64_n1024 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 608.250 / 655.189 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n1024 | prepared_reciprocal | yes | 0.864 [0.849, 0.874] | 0.864 [0.845, 0.874] | 524.331 / 562.763 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n1024 | prepare_batch |  | 0.858 [0.850, 0.867] | 0.866 [0.844, 0.873] | 518.490 / 564.354 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l1_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.005 / 0.005 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l1_n16 | word_carry | yes | 1.063 [1.059, 1.076] | 1.066 [1.054, 1.076] | 0.005 / 0.005 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_uint_add | l1_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.213 / 0.226 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l1_n1024 | word_carry | yes | 1.001 [0.986, 1.015] | 1.001 [0.991, 1.011] | 0.214 / 0.225 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_add | l2_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.011 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l2_n16 | word_carry | yes | 1.027 [1.023, 1.036] | 1.023 [1.003, 1.042] | 0.012 / 0.013 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_uint_add | l2_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.659 / 0.700 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l2_n1024 | word_carry | yes | 1.001 [0.995, 1.008] | 0.997 [0.986, 1.008] | 0.660 / 0.701 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l4_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l4_n16 | word_carry | yes | 0.784 [0.778, 0.792] | 0.792 [0.784, 0.810] | 0.013 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l4_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.941 / 0.978 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l4_n1024 | word_carry | yes | 0.743 [0.740, 0.748] | 0.749 [0.738, 0.764] | 0.700 / 0.735 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l9_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.057 / 0.060 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l9_n16 | word_carry | yes | 0.909 [0.896, 0.926] | 0.904 [0.889, 0.925] | 0.052 / 0.054 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l9_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.557 / 3.728 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l9_n1024 | word_carry | yes | 0.899 [0.893, 0.902] | 0.903 [0.891, 0.912] | 3.192 / 3.336 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l1_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.005 / 0.005 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l1_n16 | word_carry | yes | 1.068 [1.056, 1.073] | 1.051 [1.039, 1.068] | 0.005 / 0.005 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_uint_sub | l1_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.211 / 0.223 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l1_n1024 | word_carry | yes | 0.999 [0.997, 1.010] | 0.998 [0.982, 1.013] | 0.211 / 0.225 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l2_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l2_n16 | word_carry | yes | 1.027 [1.019, 1.033] | 1.021 [0.790, 1.033] | 0.012 / 0.013 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_uint_sub | l2_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.659 / 0.697 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l2_n1024 | word_carry | yes | 1.000 [0.996, 1.009] | 1.003 [0.995, 1.010] | 0.660 / 0.699 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l4_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.022 / 0.023 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l4_n16 | word_carry | yes | 1.003 [0.999, 1.011] | 1.000 [0.989, 1.011] | 0.022 / 0.023 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l4_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.333 / 1.424 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l4_n1024 | word_carry | yes | 1.008 [1.002, 1.015] | 1.005 [0.998, 1.017] | 1.343 / 1.430 | 0.094 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l9_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.062 / 0.066 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l9_n16 | word_carry | yes | 1.009 [0.990, 1.034] | 1.016 [0.988, 1.030] | 0.063 / 0.067 | 0.003 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l9_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.929 / 4.069 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l9_n1024 | word_carry | yes | 0.982 [0.978, 0.987] | 0.989 [0.975, 1.011] | 3.867 / 4.005 | 0.211 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_checked_add | l1_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.019 / 0.020 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n16 | word_option | yes | 0.478 [0.471, 0.481] | 0.485 [0.476, 0.495] | 0.009 / 0.010 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.488 / 0.517 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n1024 | word_option | yes | 1.008 [0.994, 1.013] | 1.002 [0.992, 1.014] | 0.489 / 0.518 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_checked_add | l2_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.014 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l2_n16 | word_option | yes | 1.615 [1.601, 1.625] | 1.392 [1.287, 1.589] | 0.022 / 0.024 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_add | l2_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.711 / 0.764 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l2_n1024 | word_option | yes | 1.736 [1.667, 1.746] | 1.718 [1.671, 1.742] | 1.228 / 1.318 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_add | l4_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.045 / 0.048 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n16 | word_option | yes | 0.609 [0.600, 0.630] | 0.613 [0.606, 0.653] | 0.027 / 0.029 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.534 / 3.698 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n1024 | word_option | yes | 0.424 [0.419, 0.432] | 0.435 [0.422, 0.442] | 1.497 / 1.595 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.082 / 0.084 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n16 | word_option | yes | 0.731 [0.722, 0.735] | 0.741 [0.731, 0.746] | 0.060 / 0.062 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 7.036 / 7.596 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n1024 | word_option | yes | 0.572 [0.565, 0.576] | 0.579 [0.567, 0.607] | 4.017 / 4.310 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.018 / 0.019 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n16 | word_option | yes | 0.688 [0.680, 0.695] | 0.681 [0.675, 0.695] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.459 / 0.491 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n1024 | word_option | yes | 1.432 [1.424, 1.445] | 1.427 [1.404, 1.441] | 0.659 / 0.694 | 0.023 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_sub | l2_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.023 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l2_n16 | word_option | yes | 1.032 [1.019, 1.041] | 1.038 [0.949, 1.072] | 0.022 / 0.023 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_sub | l2_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.191 / 1.271 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l2_n1024 | word_option | yes | 1.014 [0.990, 1.022] | 1.023 [0.982, 1.042] | 1.204 / 1.294 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_checked_sub | l4_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.044 / 0.046 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n16 | word_option | yes | 0.608 [0.598, 0.613] | 0.636 [0.607, 0.648] | 0.026 / 0.029 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.463 / 3.725 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n1024 | word_option | yes | 0.519 [0.503, 0.556] | 0.542 [0.519, 0.559] | 1.821 / 1.993 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.091 / 0.095 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n16 | word_option | yes | 0.718 [0.693, 0.725] | 0.721 [0.708, 0.739] | 0.065 / 0.069 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 7.249 / 7.719 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n1024 | word_option | yes | 0.567 [0.564, 0.577] | 0.573 [0.562, 0.594] | 4.133 / 4.463 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l1_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.022 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l1_n16 | word_option | yes | 0.579 [0.560, 0.592] | 0.590 [0.569, 0.601] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l1_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.220 / 1.341 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l1_n1024 | word_option | yes | 0.577 [0.564, 0.603] | 0.569 [0.500, 0.590] | 0.703 / 0.750 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l2_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.022 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l2_n16 | word_option | yes | 0.816 [0.802, 0.844] | 0.828 [0.807, 0.853] | 0.017 / 0.018 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l2_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.642 / 1.783 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l2_n1024 | word_option | yes | 0.471 [0.464, 0.487] | 0.476 [0.465, 0.510] | 0.767 / 0.831 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l4_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.047 / 0.049 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l4_n16 | word_option | yes | 0.472 [0.465, 0.492] | 0.499 [0.476, 0.516] | 0.022 / 0.025 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l4_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.516 / 3.795 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l4_n1024 | word_option | yes | 0.341 [0.328, 0.353] | 0.361 [0.332, 0.367] | 1.193 / 1.293 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l9_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.093 / 0.096 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l9_n16 | word_option | yes | 0.571 [0.562, 0.586] | 0.579 [0.567, 0.597] | 0.053 / 0.056 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l9_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 7.309 / 7.631 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l9_n1024 | word_option | yes | 0.455 [0.453, 0.462] | 0.470 [0.457, 0.486] | 3.334 / 3.609 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.017 / 0.018 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n16 | word_option | yes | 0.706 [0.704, 0.717] | 0.713 [0.708, 0.724] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.061 / 1.467 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n1024 | word_option | yes | 0.678 [0.629, 0.702] | 0.552 [0.470, 0.659] | 0.701 / 0.758 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.022 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n16 | word_option | yes | 0.821 [0.812, 0.829] | 0.833 [0.821, 0.847] | 0.017 / 0.018 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.396 / 1.712 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n1024 | word_option | yes | 0.538 [0.514, 0.591] | 0.469 [0.451, 0.507] | 0.764 / 0.818 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l4_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.051 / 0.055 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l4_n16 | word_option | yes | 0.494 [0.491, 0.498] | 0.514 [0.494, 0.527] | 0.025 / 0.028 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l4_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.936 / 4.116 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l4_n1024 | word_option | yes | 0.353 [0.342, 0.362] | 0.362 [0.351, 0.370] | 1.382 / 1.482 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l9_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.112 / 0.116 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l9_n16 | word_option | yes | 0.577 [0.561, 0.588] | 0.578 [0.573, 0.586] | 0.065 / 0.067 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l9_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.515 / 8.817 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l9_n1024 | word_option | yes | 0.467 [0.460, 0.471] | 0.481 [0.470, 0.487] | 3.969 / 4.235 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a1_b1_n16 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.006 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a1_b1_n16 | schoolbook |  | 1.170 [1.149, 1.174] | 1.167 [1.137, 1.178] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a1_b1_n1024 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.331 / 0.348 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a1_b1_n1024 | schoolbook |  | 1.150 [1.136, 1.190] | 1.141 [1.120, 1.192] | 0.382 / 0.402 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a2_b2_n16 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.023 / 0.024 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a2_b2_n16 | schoolbook |  | 1.019 [1.012, 1.024] | 1.016 [1.006, 1.031] | 0.023 / 0.024 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a2_b2_n1024 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.334 / 1.386 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a2_b2_n1024 | schoolbook |  | 1.004 [0.996, 1.009] | 1.004 [0.989, 1.024] | 1.336 / 1.398 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | opt_exact_product | a4_b4_n16 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.102 / 0.107 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a4_b4_n16 | schoolbook |  | 1.043 [1.033, 1.049] | 1.046 [1.037, 1.066] | 0.107 / 0.114 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a4_b4_n1024 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6.358 / 6.683 | 0.125 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a4_b4_n1024 | schoolbook |  | 1.024 [1.012, 1.032] | 1.009 [0.994, 1.044] | 6.516 / 6.874 | 0.125 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a9_b9_n16 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.786 / 0.842 | 0.004 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a9_b9_n16 | schoolbook |  | 1.218 [1.201, 1.245] | 1.272 [1.247, 1.311] | 0.963 / 1.070 | 0.004 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a9_b9_n1024 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 50.226 / 54.183 | 0.281 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a9_b9_n1024 | schoolbook |  | 1.231 [1.196, 1.268] | 1.306 [1.221, 1.319] | 61.813 / 68.111 | 0.281 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a2_b9_n16 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.115 / 0.121 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a2_b9_n16 | schoolbook |  | 0.993 [0.987, 1.001] | 0.995 [0.954, 1.007] | 0.114 / 0.120 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a2_b9_n1024 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 7.511 / 7.898 | 0.172 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a2_b9_n1024 | schoolbook |  | 0.936 [0.927, 0.941] | 0.938 [0.926, 0.947] | 7.018 / 7.435 | 0.172 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a32_b32_n16 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 10.452 / 11.253 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a32_b32_n16 | schoolbook |  | 1.291 [1.285, 1.302] | 1.287 [1.277, 1.309] | 13.466 / 14.319 | 0.016 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a32_b32_n1024 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 673.875 / 711.653 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a32_b32_n1024 | schoolbook |  | 1.291 [1.282, 1.304] | 1.286 [1.266, 1.305] | 871.339 / 918.356 | 1.000 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a64_b64_n16 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 36.490 / 38.709 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a64_b64_n16 | schoolbook |  | 1.718 [1.707, 1.732] | 1.724 [1.685, 1.742] | 62.709 / 66.314 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a64_b64_n1024 | crypto_bigint |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2376.385 / 2491.388 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a64_b64_n1024 | schoolbook |  | 1.706 [1.691, 1.714] | 1.688 [1.655, 1.735] | 4034.427 / 4195.057 | 2.000 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l2_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.078 / 0.082 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n16 | fused_exact | yes | 0.803 [0.798, 0.811] | 0.803 [0.795, 0.817] | 0.062 / 0.065 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.854 / 5.140 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | fused_exact | yes | 0.785 [0.778, 0.790] | 0.785 [0.781, 0.797] | 3.820 / 3.995 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.226 / 0.246 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | fused_exact | yes | 0.967 [0.964, 0.977] | 0.967 [0.954, 0.982] | 0.219 / 0.239 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 14.278 / 15.039 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | fused_exact | yes | 0.968 [0.962, 0.976] | 0.969 [0.952, 0.994] | 13.865 / 14.760 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.083 / 1.137 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n16 | fused_exact |  | 1.345 [1.333, 1.354] | 1.336 [1.316, 1.354] | 1.455 / 1.554 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l9_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 68.940 / 73.829 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n1024 | fused_exact |  | 1.320 [1.309, 1.329] | 1.311 [1.300, 1.328] | 90.848 / 97.740 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_exact_unsigned_mac | l2_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.040 / 0.044 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l2_n16 | fused_exact | yes | 0.898 [0.894, 0.905] | 0.898 [0.874, 0.916] | 0.036 / 0.040 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l2_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.495 / 2.717 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l2_n1024 | fused_exact | yes | 0.878 [0.873, 0.888] | 0.878 [0.861, 0.892] | 2.188 / 2.352 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l4_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.148 / 0.156 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l4_n16 | fused_exact | yes | 1.029 [1.022, 1.035] | 1.025 [1.015, 1.035] | 0.152 / 0.160 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_exact_unsigned_mac | l4_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 9.343 / 9.888 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l4_n1024 | fused_exact | yes | 1.031 [1.027, 1.038] | 1.027 [1.012, 1.034] | 9.640 / 10.172 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_exact_unsigned_mac | l9_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.890 / 0.922 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l9_n16 | fused_exact |  | 1.446 [1.434, 1.458] | 1.434 [1.419, 1.449] | 1.282 / 1.338 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_exact_unsigned_mac | l9_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 56.904 / 62.024 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l9_n1024 | fused_exact |  | 1.406 [1.396, 1.416] | 1.386 [1.360, 1.417] | 80.061 / 86.558 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_prime_dot | q100_n16 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.059 / 0.063 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n16 | acc4 |  | 1.052 [1.043, 1.062] | 1.044 [1.032, 1.065] | 0.062 / 0.066 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_prime_dot | q100_n16 | length_dispatch | yes | 1.003 [0.995, 1.013] | 1.008 [0.980, 1.020] | 0.059 / 0.063 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_dot | q100_n1024 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.435 / 2.629 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n1024 | acc4 |  | 0.990 [0.982, 0.996] | 0.994 [0.952, 1.004] | 2.404 / 2.602 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n1024 | length_dispatch | yes | 0.990 [0.982, 0.995] | 0.985 [0.955, 1.000] | 2.402 / 2.598 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n65536 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 155.477 / 168.713 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n65536 | acc4 |  | 0.984 [0.974, 0.992] | 0.986 [0.965, 1.017] | 153.024 / 165.554 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_dot | q100_n65536 | length_dispatch | yes | 0.985 [0.979, 0.994] | 0.994 [0.973, 1.005] | 153.192 / 164.471 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n16 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.059 / 0.065 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n16 | acc4 |  | 1.042 [1.038, 1.057] | 1.044 [1.034, 1.078] | 0.062 / 0.069 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_prime_dot | q128_n16 | length_dispatch | yes | 1.009 [0.999, 1.015] | 1.002 [0.988, 1.026] | 0.060 / 0.065 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_dot | q128_n1024 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.438 / 2.702 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n1024 | acc4 |  | 0.985 [0.982, 0.993] | 0.974 [0.891, 0.997] | 2.406 / 2.623 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n1024 | length_dispatch | yes | 0.984 [0.978, 0.993] | 0.973 [0.736, 0.996] | 2.404 / 2.601 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n65536 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 155.247 / 170.887 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n65536 | acc4 |  | 0.986 [0.978, 0.998] | 0.986 [0.944, 1.009] | 153.366 / 169.066 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n65536 | length_dispatch | yes | 0.990 [0.982, 0.995] | 0.987 [0.950, 1.009] | 153.413 / 165.288 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n16 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.041 / 0.043 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n16 | acc4 | yes | 0.999 [0.989, 1.009] | 1.000 [0.983, 1.017] | 0.041 / 0.043 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_linear | q100_n1024 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.525 / 1.618 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n1024 | acc4 | yes | 0.886 [0.877, 0.893] | 0.891 [0.880, 0.902] | 1.353 / 1.438 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n65536 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 96.411 / 103.472 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n65536 | acc4 | yes | 0.869 [0.855, 0.876] | 0.866 [0.856, 0.887] | 83.423 / 90.187 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n16 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.041 / 0.044 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n16 | acc4 | yes | 0.995 [0.987, 1.009] | 1.002 [0.984, 1.019] | 0.041 / 0.044 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_linear | q128_n1024 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.526 / 1.662 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n1024 | acc4 | yes | 0.888 [0.882, 0.892] | 0.890 [0.880, 0.919] | 1.352 / 1.493 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n65536 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 96.059 / 102.971 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n65536 | acc4 | yes | 0.869 [0.857, 0.879] | 0.867 [0.852, 0.898] | 83.509 / 90.754 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.172 / 1.246 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | compact_vartime |  | 0.989 [0.977, 1.001] | 0.987 [0.971, 1.004] | 1.158 / 1.221 | 0.000 | 2 / 1536 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | compact_ct |  | 1.284 [1.277, 1.294] | 1.270 [1.266, 1.330] | 1.502 / 1.596 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 26.783 / 29.095 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_vartime |  | 0.967 [0.959, 0.972] | 0.968 [0.931, 0.982] | 25.824 / 27.689 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_ct |  | 0.980 [0.973, 0.986] | 0.973 [0.946, 0.989] | 26.191 / 28.312 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.034 / 1.100 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | compact_vartime |  | 1.127 [1.113, 1.138] | 1.120 [1.100, 1.138] | 1.158 / 1.232 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | compact_ct |  | 1.469 [1.447, 1.477] | 1.442 [1.425, 1.459] | 1.501 / 1.614 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.030 / 22.582 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_vartime |  | 1.233 [1.223, 1.244] | 1.224 [1.215, 1.251] | 25.846 / 27.847 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_ct |  | 1.249 [1.237, 1.257] | 1.239 [1.229, 1.257] | 26.195 / 28.358 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n16 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.087 / 0.094 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q100_zero_n16 | compact_vartime |  | 11.214 [11.062, 11.359] | 10.985 [10.818, 11.284] | 0.985 / 1.035 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n16 | compact_ct |  | 17.155 [17.031, 17.252] | 16.940 [16.517, 17.079] | 1.498 / 1.593 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.291 / 4.618 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_vartime |  | 6.008 [5.961, 6.075] | 5.953 [5.835, 6.070] | 25.659 / 27.860 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_ct |  | 6.174 [6.092, 6.206] | 6.081 [5.987, 6.189] | 26.248 / 28.418 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.320 / 1.411 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | compact_vartime |  | 0.990 [0.981, 1.004] | 1.001 [0.981, 1.011] | 1.311 / 1.412 | 0.000 | 2 / 1536 | inconclusive |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | compact_ct |  | 1.148 [1.130, 1.151] | 1.144 [1.131, 1.172] | 1.503 / 1.614 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 27.258 / 28.791 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_vartime |  | 0.960 [0.952, 0.976] | 0.964 [0.954, 0.990] | 26.159 / 28.353 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_ct |  | 0.971 [0.962, 0.991] | 0.965 [0.957, 0.988] | 26.502 / 28.143 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.189 / 1.265 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | compact_vartime |  | 1.112 [1.101, 1.121] | 1.102 [1.080, 1.116] | 1.319 / 1.391 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | compact_ct |  | 1.277 [1.260, 1.288] | 1.259 [1.239, 1.271] | 1.514 / 1.604 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.349 / 22.560 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_vartime |  | 1.231 [1.217, 1.238] | 1.226 [1.217, 1.264] | 26.174 / 28.158 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_ct |  | 1.233 [1.226, 1.243] | 1.226 [1.217, 1.238] | 26.332 / 28.103 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n16 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.088 / 0.095 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q128_zero_n16 | compact_vartime |  | 14.963 [14.840, 15.137] | 14.734 [14.615, 15.015] | 1.306 / 1.412 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n16 | compact_ct |  | 17.138 [17.025, 17.254] | 17.014 [16.818, 17.130] | 1.494 / 1.618 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.271 / 4.703 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_vartime |  | 6.139 [6.086, 6.167] | 6.076 [5.873, 6.139] | 26.000 / 28.284 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_ct |  | 6.182 [6.130, 6.216] | 6.093 [5.901, 6.170] | 26.211 / 28.558 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n16 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.081 / 1.164 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n16 | compact_ct |  | 1.333 [1.321, 1.343] | 1.326 [1.307, 1.466] | 1.439 / 1.547 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n1024 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.436 / 26.725 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n1024 | compact_ct |  | 1.018 [1.013, 1.024] | 1.010 [0.992, 1.032] | 24.771 / 27.346 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n16 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.083 / 1.171 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n16 | compact_ct |  | 1.331 [1.321, 1.340] | 1.318 [1.296, 1.334] | 1.436 / 1.544 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n1024 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.522 / 26.053 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n1024 | compact_ct |  | 1.017 [1.014, 1.027] | 1.015 [0.990, 1.027] | 24.884 / 26.803 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n16 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.906 / 0.979 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n16 | compact_ct |  | 1.589 [1.582, 1.602] | 1.576 [1.560, 1.601] | 1.440 / 1.539 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n1024 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.348 / 26.616 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n1024 | compact_ct |  | 1.025 [1.018, 1.031] | 1.025 [1.009, 1.037] | 24.962 / 27.182 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n16 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.240 / 1.342 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n16 | compact_ct |  | 1.161 [1.147, 1.174] | 1.164 [1.157, 1.175] | 1.444 / 1.567 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n1024 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.821 / 26.129 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n1024 | compact_ct |  | 1.010 [0.994, 1.017] | 1.008 [0.984, 1.019] | 25.039 / 26.155 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n16 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.241 / 1.306 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n16 | compact_ct |  | 1.165 [1.159, 1.178] | 1.173 [1.159, 1.187] | 1.446 / 1.536 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n1024 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.688 / 26.534 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n1024 | compact_ct |  | 1.014 [1.001, 1.021] | 1.006 [0.990, 1.023] | 24.772 / 26.721 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n16 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.235 / 1.361 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n16 | compact_ct |  | 1.169 [1.148, 1.180] | 1.165 [1.148, 1.173] | 1.441 / 1.592 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n1024 | compact_vartime |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.574 / 26.732 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n1024 | compact_ct |  | 1.018 [1.010, 1.023] | 1.017 [1.001, 1.031] | 25.007 / 27.031 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_prime_public_pow | e17_n16 | bounded_window |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6.791 / 7.428 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e17_n16 | public_binary | yes | 0.374 [0.372, 0.376] | 0.383 [0.368, 0.390] | 2.538 / 2.801 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e17_n1024 | bounded_window |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 432.656 / 469.125 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e17_n1024 | public_binary | yes | 0.374 [0.373, 0.377] | 0.384 [0.374, 0.393] | 162.297 / 177.096 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e127_n16 | bounded_window |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 31.186 / 33.757 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e127_n16 | public_binary | yes | 0.715 [0.709, 0.717] | 0.711 [0.703, 0.723] | 22.249 / 23.962 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e127_n1024 | bounded_window |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2021.073 / 2152.221 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e127_n1024 | public_binary | yes | 0.713 [0.707, 0.717] | 0.721 [0.708, 0.726] | 1434.844 / 1551.340 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n16 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n16 | prepared_formula |  | 1.009 [1.003, 1.014] | 1.004 [0.990, 1.018] | 0.013 / 0.013 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | zero_n16 | public_scalar_dispatch | yes | 0.600 [0.594, 0.604] | 0.607 [0.598, 0.615] | 0.008 / 0.008 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n1024 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.746 / 0.821 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n1024 | prepared_formula |  | 1.002 [0.996, 1.011] | 1.013 [0.993, 1.022] | 0.751 / 0.831 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | zero_n1024 | public_scalar_dispatch | yes | 0.459 [0.454, 0.462] | 0.469 [0.455, 0.482] | 0.342 / 0.388 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n65536 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 48.110 / 53.470 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n65536 | prepared_formula |  | 1.011 [0.997, 1.018] | 1.013 [0.988, 1.045] | 48.347 / 54.376 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | zero_n65536 | public_scalar_dispatch | yes | 0.443 [0.439, 0.457] | 0.466 [0.443, 0.486] | 21.419 / 24.970 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n16 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n16 | prepared_formula |  | 1.006 [1.003, 1.026] | 1.013 [0.992, 1.031] | 0.013 / 0.014 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | half_n16 | public_scalar_dispatch | yes | 0.734 [0.727, 0.738] | 0.739 [0.721, 0.758] | 0.009 / 0.010 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n1024 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.747 / 0.802 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n1024 | prepared_formula |  | 0.997 [0.989, 1.006] | 1.006 [0.988, 1.021] | 0.746 / 0.797 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | half_n1024 | public_scalar_dispatch | yes | 0.611 [0.606, 0.618] | 0.626 [0.598, 0.634] | 0.459 / 0.492 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n65536 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 48.279 / 52.472 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n65536 | prepared_formula |  | 1.010 [0.998, 1.019] | 1.018 [0.992, 1.046] | 48.693 / 54.636 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | half_n65536 | public_scalar_dispatch | yes | 0.611 [0.605, 0.628] | 0.636 [0.613, 0.676] | 29.486 / 34.926 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | full_n16 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | full_n16 | prepared_formula |  | 1.008 [0.998, 1.014] | 1.006 [0.987, 1.026] | 0.013 / 0.014 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | full_n16 | public_scalar_dispatch |  | 1.178 [1.165, 1.187] | 1.167 [1.152, 1.195] | 0.015 / 0.016 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_gf_fixed | full_n1024 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.757 / 0.812 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | full_n1024 | prepared_formula |  | 0.998 [0.992, 1.009] | 1.003 [0.981, 1.024] | 0.755 / 0.827 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | full_n1024 | public_scalar_dispatch |  | 1.116 [1.106, 1.124] | 1.125 [1.099, 1.352] | 0.843 / 0.927 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_gf_fixed | full_n65536 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 48.915 / 52.231 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | full_n65536 | prepared_formula |  | 1.006 [0.996, 1.013] | 1.035 [1.004, 1.070] | 49.185 / 54.253 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | full_n65536 | public_scalar_dispatch |  | 1.113 [1.104, 1.122] | 1.122 [1.098, 1.179] | 54.324 / 59.717 | 2.000 | 0 / 0 | regression |
| aarch64 | opt_gf_butterfly | zero_n16 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | zero_n16 | public_scalar_dispatch | yes | 0.848 [0.835, 0.850] | 0.850 [0.828, 0.857] | 0.013 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | zero_n1024 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.927 / 1.019 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | zero_n1024 | public_scalar_dispatch | yes | 0.832 [0.827, 0.844] | 0.842 [0.818, 0.853] | 0.777 / 0.866 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | zero_n65536 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 60.108 / 63.558 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | zero_n65536 | public_scalar_dispatch | yes | 0.833 [0.824, 0.838] | 0.835 [0.819, 0.853] | 49.963 / 52.950 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n16 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n16 | public_scalar_dispatch | yes | 0.860 [0.832, 0.867] | 0.867 [0.831, 0.885] | 0.014 / 0.015 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n1024 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.923 / 0.991 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n1024 | public_scalar_dispatch | yes | 0.741 [0.737, 0.754] | 0.751 [0.737, 0.768] | 0.686 / 0.751 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n65536 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 60.134 / 67.599 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n65536 | public_scalar_dispatch | yes | 0.745 [0.730, 0.797] | 0.773 [0.747, 0.826] | 44.937 / 53.258 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | full_n16 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | full_n16 | public_scalar_dispatch |  | 1.229 [1.194, 1.235] | 1.210 [1.157, 1.231] | 0.019 / 0.021 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_gf_butterfly | full_n1024 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.923 / 1.005 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | full_n1024 | public_scalar_dispatch |  | 1.165 [1.157, 1.177] | 1.178 [1.136, 1.270] | 1.081 / 1.193 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_gf_butterfly | full_n65536 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 60.618 / 68.508 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | full_n65536 | public_scalar_dispatch |  | 1.168 [1.138, 1.174] | 1.158 [1.108, 1.172] | 70.713 / 76.663 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_gf_round | 16 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.095 / 0.101 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 16 | wide1 | yes | 0.812 [0.809, 0.817] | 0.815 [0.777, 0.827] | 0.077 / 0.081 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 16 | wide2 |  | 0.876 [0.870, 0.881] | 0.876 [0.850, 0.902] | 0.083 / 0.088 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 1024 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 5.741 / 6.000 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 1024 | wide1 | yes | 0.816 [0.810, 0.829] | 0.817 [0.806, 0.834] | 4.685 / 4.956 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 1024 | wide2 |  | 0.846 [0.842, 0.854] | 0.859 [0.847, 0.895] | 4.868 / 5.241 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 65536 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 364.858 / 389.455 | 5.000 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 65536 | wide1 | yes | 0.819 [0.807, 0.831] | 0.818 [0.804, 0.841] | 296.508 / 320.436 | 5.000 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 65536 | wide2 |  | 0.855 [0.848, 0.863] | 0.857 [0.850, 0.868] | 312.385 / 333.874 | 5.000 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 16 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.024 / 0.025 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 16 | native_batch | yes | 0.134 [0.133, 0.135] | 0.136 [0.133, 0.145] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 1024 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.481 / 1.530 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 1024 | native_batch | yes | 0.046 [0.045, 0.046] | 0.046 [0.045, 0.048] | 0.068 / 0.071 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 65536 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 95.398 / 100.658 | 0.188 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 65536 | native_batch | yes | 0.046 [0.046, 0.047] | 0.047 [0.047, 0.049] | 4.388 / 4.829 | 0.188 | 0 / 0 | pass |
| aarch64 | opt_phi8 | 16 | table_public_input |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.009 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_phi8 | 16 | fixed_basis |  | 11.101 [8.344, 11.339] | 10.866 [8.293, 11.262] | 0.094 / 0.100 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_phi8 | 1024 | table_public_input |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.371 / 0.394 | 0.017 | 0 / 0 | pass |
| aarch64 | opt_phi8 | 1024 | fixed_basis |  | 15.754 [15.714, 15.892] | 15.571 [15.227, 15.799] | 5.873 / 6.151 | 0.017 | 0 / 0 | regression |
| aarch64 | opt_phi8 | 65536 | table_public_input |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 23.291 / 26.689 | 1.062 | 0 / 0 | pass |
| aarch64 | opt_phi8 | 65536 | fixed_basis |  | 16.241 [16.115, 16.438] | 15.632 [14.809, 16.109] | 377.458 / 416.153 | 1.062 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.019 / 0.020 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_b127_mul | 16 | karatsuba |  | 1.296 [1.290, 1.301] | 1.303 [1.282, 1.309] | 0.025 / 0.026 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 16 | pfold |  | 1.292 [1.287, 1.300] | 1.289 [1.270, 1.300] | 0.025 / 0.026 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.227 / 1.283 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_b127_mul | 1024 | karatsuba |  | 1.299 [1.283, 1.305] | 1.285 [1.272, 1.308] | 1.586 / 1.667 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 1024 | pfold |  | 1.301 [1.292, 1.314] | 1.306 [1.294, 1.350] | 1.600 / 1.678 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 65536 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 79.408 / 85.976 | 3.000 | 0 / 0 | pass |
| aarch64 | opt_b127_mul | 65536 | karatsuba |  | 1.297 [1.285, 1.304] | 1.283 [1.250, 1.302] | 102.585 / 108.915 | 3.000 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 65536 | pfold |  | 1.308 [1.299, 1.316] | 1.295 [1.259, 1.309] | 103.608 / 109.466 | 3.000 | 0 / 0 | regression |
| aarch64 | opt_ood | log10 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.088 / 3.246 | 0.016 | 13 / 32832 | pass |
| aarch64 | opt_ood | log10 | reuse_products | yes | 0.722 [0.713, 0.731] | 0.717 [0.708, 0.746] | 2.235 / 2.393 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log16 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 67.993 / 80.981 | 1.000 | 19 / 131808 | pass |
| aarch64 | opt_ood | log16 | reuse_products | yes | 0.865 [0.857, 0.889] | 0.905 [0.863, 0.980] | 59.295 / 71.066 | 1.000 | 2 / 65792 | pass |
| aarch64 | opt_ood | log18 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 221.471 / 253.364 | 4.000 | 21 / 134112 | pass |
| aarch64 | opt_ood | log18 | reuse_products | yes | 0.891 [0.885, 0.907] | 0.889 [0.847, 0.925] | 199.509 / 229.732 | 4.000 | 3 / 68080 | pass |
| aarch64 | opt_ood_reuse | log10 | allocate |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.230 / 2.340 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood_reuse | log10 | scratch | yes | 0.822 [0.817, 0.828] | 0.834 [0.813, 0.845] | 1.832 / 1.914 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_ood_reuse | log16 | allocate |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 59.850 / 68.232 | 1.000 | 2 / 65792 | pass |
| aarch64 | opt_ood_reuse | log16 | scratch | yes | 0.976 [0.953, 0.995] | 0.963 [0.928, 1.004] | 58.574 / 66.044 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_ood_reuse | log18 | allocate |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 196.897 / 226.634 | 4.000 | 2 / 66560 | pass |
| aarch64 | opt_ood_reuse | log18 | scratch | yes | 0.991 [0.982, 1.002] | 0.990 [0.966, 1.163] | 196.941 / 236.237 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log8_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 47.038 / 57.819 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | depth_first | yes | 0.908 [0.899, 0.919] | 0.906 [0.842, 0.947] | 42.866 / 51.116 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 420.680 / 511.465 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | depth_first | yes | 0.891 [0.873, 0.903] | 0.882 [0.842, 0.910] | 377.112 / 414.339 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log15_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3849.521 / 6378.003 | 12.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log15_lanes8 | depth_first | yes | 0.942 [0.933, 0.958] | 0.955 [0.892, 1.135] | 3623.666 / 6514.212 | 12.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log15_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 11282.854 / 12619.890 | 48.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log15_lanes32 | depth_first | yes | 0.939 [0.928, 0.952] | 0.982 [0.720, 0.996] | 10646.291 / 12223.484 | 48.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log16_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 23764.521 / 25971.952 | 96.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log16_lanes32 | depth_first | yes | 0.969 [0.958, 0.983] | 0.996 [0.957, 1.031] | 23231.416 / 25839.406 | 96.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 106698.479 / 122601.368 | 384.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log18_lanes32 | depth_first | yes | 0.960 [0.952, 1.011] | 0.957 [0.819, 1.141] | 104504.083 / 117146.807 | 384.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log8_lanes1 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.600 / 3.357 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | 1.506 [1.456, 1.554] | 1.598 [1.517, 2.224] | 3.947 / 5.226 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_pack | logs9_9 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 25.948 / 31.761 | 0.016 | 1 / 16384 | pass |
| aarch64 | opt_pack | logs9_9 | single_write | yes | 0.027 [0.026, 0.180] | 0.027 [0.021, 0.169] | 0.723 / 0.790 | 0.016 | 1 / 16384 | pass |
| aarch64 | opt_pack | logs16_14 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 125.007 / 152.627 | 2.000 | 2 / 2098672 | pass |
| aarch64 | opt_pack | logs16_14 | single_write | yes | 0.750 [0.712, 0.882] | 0.736 [0.643, 0.848] | 95.532 / 103.470 | 2.000 | 1 / 2097152 | pass |
| aarch64 | opt_pack | logs18_18 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 409.318 / 464.937 | 8.000 | 1 / 8388608 | pass |
| aarch64 | opt_pack | logs18_18 | single_write | yes | 1.081 [1.064, 1.132] | 1.070 [1.021, 1.130] | 442.070 / 493.973 | 8.000 | 1 / 8388608 | regression |
| aarch64 | opt_packed_ood | logs9_9 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 17.360 / 48.428 | 0.031 | 14 / 49216 | pass |
| aarch64 | opt_packed_ood | logs9_9 | single_write_ood | yes | 0.233 [0.072, 0.364] | 0.087 [0.059, 0.286] | 2.959 / 3.353 | 0.031 | 3 / 32784 | pass |
| aarch64 | opt_packed_ood | logs16_14 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 262.056 / 298.706 | 4.000 | 21 / 2229728 | pass |
| aarch64 | opt_packed_ood | logs16_14 | single_write_ood | yes | 1.109 [0.895, 1.125] | 0.992 [0.896, 1.125] | 293.146 / 321.802 | 4.000 | 3 / 2163200 | inconclusive |
| aarch64 | opt_packed_ood | logs18_18 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 841.219 / 930.135 | 16.000 | 23 / 8525792 | pass |
| aarch64 | opt_packed_ood | logs18_18 | single_write_ood | yes | 1.145 [1.033, 1.206] | 1.126 [1.020, 1.191] | 978.417 / 1060.308 | 16.000 | 3 / 8456192 | regression |
| aarch64 | opt_gf_two_pair | 16 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.183 / 0.188 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_gf_two_pair | 16 | separate_wide | yes | 0.830 [0.820, 0.833] | 0.837 [0.825, 0.846] | 0.152 / 0.158 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_gf_two_pair | 1024 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 11.200 / 11.904 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_gf_two_pair | 1024 | separate_wide | yes | 0.829 [0.825, 0.841] | 0.838 [0.829, 0.855] | 9.326 / 9.913 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_gf_fold_round | 16 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.185 / 0.192 | 0.004 | 0 / 0 | pass |
| aarch64 | opt_gf_fold_round | 16 | fold_then_wide | yes | 0.922 [0.918, 0.927] | 0.926 [0.912, 0.953] | 0.171 / 0.178 | 0.004 | 0 / 0 | pass |
| aarch64 | opt_gf_fold_round | 1024 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 11.952 / 12.749 | 0.250 | 0 / 0 | pass |
| aarch64 | opt_gf_fold_round | 1024 | fold_then_wide | yes | 0.945 [0.922, 0.950] | 0.936 [0.915, 0.952] | 11.261 / 11.969 | 0.250 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.831 / 0.889 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | chunked |  | 1.015 [1.007, 1.023] | 1.020 [1.007, 1.036] | 0.843 / 0.910 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_grid | 1024 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 54.582 / 59.671 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | chunked |  | 1.042 [0.992, 1.049] | 1.040 [0.995, 1.062] | 56.099 / 61.083 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf8_inverse | 16 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.586 / 0.634 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf8_inverse | 16 | vector_chain | yes | 0.050 [0.050, 0.051] | 0.051 [0.050, 0.053] | 0.029 / 0.033 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf8_inverse | 1024 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 37.172 / 38.427 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_gf8_inverse | 1024 | vector_chain | yes | 0.044 [0.044, 0.044] | 0.044 [0.044, 0.049] | 1.628 / 1.729 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_gf8_inverse | 65536 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2397.615 / 2552.029 | 0.125 | 0 / 0 | pass |
| aarch64 | opt_gf8_inverse | 65536 | vector_chain | yes | 0.043 [0.043, 0.044] | 0.044 [0.043, 0.045] | 104.875 / 113.351 | 0.125 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | fixed_schedule | yes | 0.474 [0.472, 0.483] | 0.489 [0.469, 0.503] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.652 / 0.682 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | fixed_schedule | yes | 0.384 [0.381, 0.388] | 0.392 [0.384, 0.397] | 0.251 / 0.265 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | fixed_schedule | yes | 0.454 [0.412, 0.589] | 0.460 [0.408, 0.590] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.746 / 0.854 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | fixed_schedule | yes | 0.335 [0.315, 0.340] | 0.344 [0.312, 0.353] | 0.250 / 0.271 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.141 / 0.147 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | fixed_schedule | yes | 0.609 [0.604, 0.614] | 0.618 [0.604, 0.623] | 0.086 / 0.091 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.248 / 8.571 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | fixed_schedule | yes | 0.795 [0.790, 0.803] | 0.796 [0.789, 0.817] | 6.572 / 6.850 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.056 / 0.065 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | fixed_schedule | yes | 1.512 [1.335, 1.613] | 1.499 [1.330, 1.611] | 0.086 / 0.088 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.508 / 3.742 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | fixed_schedule | yes | 1.887 [1.796, 1.937] | 1.843 [1.745, 1.905] | 6.567 / 6.726 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.984 / 2.102 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | fixed_schedule | yes | 0.420 [0.414, 0.423] | 0.417 [0.400, 0.432] | 0.832 / 0.886 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 126.708 / 133.100 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | fixed_schedule | yes | 0.419 [0.416, 0.424] | 0.426 [0.410, 0.436] | 53.128 / 56.546 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.214 / 0.246 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n16 | fixed_schedule | yes | 3.998 [3.533, 4.035] | 3.911 [3.434, 3.966] | 0.840 / 0.866 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 17.003 / 31.601 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n1024 | fixed_schedule | yes | 3.109 [2.546, 3.678] | 1.867 [1.702, 2.281] | 53.775 / 56.417 | 0.141 | 0 / 0 | regression |
