# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_integer_mac | l1_signed16_n16 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_signed16_n16 | products2 |  | 1.249 [1.236, 1.297] | 1.237 [1.194, 1.330] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l1_signed16_n16 | products4 |  | 1.010 [1.001, 1.104] | 1.005 [0.974, 1.153] | 0.006 / 0.007 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n16 | deferred_columns |  | 0.997 [0.989, 1.004] | 0.990 [0.952, 1.009] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_signed16_n16 | incumbent |  | 1.043 [1.035, 1.051] | 1.041 [1.012, 1.088] | 0.006 / 0.006 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l1_signed16_n16 | native_word |  | 1.007 [0.997, 1.015] | 1.004 [0.969, 1.021] | 0.006 / 0.006 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.334 / 0.363 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | products2 |  | 1.029 [1.007, 1.038] | 1.035 [0.981, 1.054] | 0.342 / 0.363 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | products4 |  | 0.999 [0.988, 1.007] | 1.001 [0.925, 1.022] | 0.333 / 0.348 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | deferred_columns |  | 0.998 [0.993, 1.007] | 1.005 [0.958, 1.053] | 0.334 / 0.351 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | incumbent |  | 1.003 [0.995, 1.010] | 1.010 [0.945, 1.089] | 0.334 / 0.382 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | native_word |  | 0.996 [0.991, 1.006] | 0.999 [0.963, 1.128] | 0.333 / 0.348 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.015 / 21.505 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | products2 |  | 0.999 [0.994, 1.005] | 1.009 [0.994, 1.018] | 20.992 / 21.634 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | products4 |  | 1.002 [0.998, 1.008] | 1.008 [0.999, 1.016] | 21.033 / 21.735 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | deferred_columns |  | 1.001 [0.997, 1.005] | 1.005 [1.000, 1.015] | 21.049 / 21.719 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | incumbent |  | 1.004 [0.994, 1.006] | 1.008 [0.993, 1.023] | 20.994 / 21.688 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | native_word |  | 1.000 [0.996, 1.005] | 1.005 [0.996, 1.014] | 21.015 / 21.661 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n16 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n16 | products2 |  | 1.249 [1.244, 1.254] | 1.242 [1.228, 1.250] | 0.007 / 0.007 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l1_full_n16 | products4 |  | 1.008 [0.999, 1.014] | 1.006 [0.989, 1.026] | 0.006 / 0.006 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n16 | deferred_columns |  | 1.026 [0.997, 1.034] | 1.027 [0.991, 1.036] | 0.006 / 0.006 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n16 | incumbent |  | 1.047 [1.041, 1.053] | 1.043 [1.034, 1.057] | 0.006 / 0.006 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l1_full_n16 | native_word |  | 1.057 [1.001, 1.071] | 1.071 [0.997, 1.087] | 0.006 / 0.006 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n1024 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.333 / 0.349 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n1024 | products2 |  | 1.014 [1.006, 1.020] | 1.010 [0.997, 1.130] | 0.337 / 0.354 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n1024 | products4 |  | 1.002 [0.993, 1.006] | 0.997 [0.988, 1.089] | 0.333 / 0.356 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n1024 | deferred_columns |  | 1.003 [0.997, 1.007] | 0.998 [0.991, 1.060] | 0.333 / 0.358 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n1024 | incumbent |  | 1.002 [0.992, 1.007] | 1.002 [0.978, 1.014] | 0.334 / 0.354 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n1024 | native_word |  | 0.999 [0.994, 1.008] | 1.000 [0.981, 1.039] | 0.333 / 0.359 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n65536 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.169 / 22.090 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n65536 | products2 |  | 0.998 [0.993, 1.003] | 0.994 [0.984, 1.007] | 21.162 / 22.027 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n65536 | products4 |  | 0.997 [0.993, 1.006] | 0.997 [0.986, 1.013] | 21.147 / 22.166 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n65536 | deferred_columns |  | 0.997 [0.994, 1.003] | 0.996 [0.939, 1.007] | 21.258 / 22.051 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n65536 | incumbent |  | 0.998 [0.992, 1.004] | 0.998 [0.975, 1.011] | 21.182 / 22.095 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n65536 | native_word |  | 0.995 [0.986, 1.005] | 0.999 [0.983, 1.012] | 21.148 / 22.115 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l2_signed16_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n16 | products2 |  | 0.891 [0.883, 0.895] | 0.892 [0.875, 0.909] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n16 | products4 |  | 0.934 [0.929, 0.942] | 0.932 [0.917, 0.964] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n16 | deferred_columns |  | 1.611 [1.577, 1.630] | 1.592 [1.550, 1.623] | 0.021 / 0.022 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_signed16_n16 | incumbent | yes | 0.837 [0.831, 0.847] | 0.846 [0.829, 0.866] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.699 / 0.742 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | products2 |  | 0.951 [0.942, 0.958] | 0.945 [0.923, 0.963] | 0.666 / 0.693 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | products4 |  | 0.953 [0.946, 0.958] | 0.960 [0.934, 0.969] | 0.666 / 0.698 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | deferred_columns |  | 1.685 [1.672, 1.694] | 1.674 [1.633, 1.706] | 1.176 / 1.226 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | incumbent | yes | 0.953 [0.944, 0.957] | 0.948 [0.928, 0.972] | 0.665 / 0.695 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 44.294 / 46.651 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | products2 |  | 0.952 [0.948, 0.959] | 0.957 [0.937, 0.980] | 42.268 / 44.396 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | products4 |  | 0.955 [0.946, 0.961] | 0.948 [0.936, 0.962] | 42.253 / 44.629 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | deferred_columns |  | 1.677 [1.660, 1.687] | 1.653 [1.624, 1.696] | 73.981 / 77.388 | 2.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | incumbent | yes | 0.951 [0.945, 0.959] | 0.941 [0.928, 0.956] | 42.169 / 44.033 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n16 | products2 |  | 0.893 [0.883, 0.902] | 0.898 [0.891, 0.915] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n16 | products4 |  | 0.933 [0.926, 0.945] | 0.936 [0.925, 0.945] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n16 | deferred_columns |  | 1.631 [1.618, 1.641] | 1.604 [1.590, 1.626] | 0.021 / 0.022 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_full_n16 | incumbent | yes | 0.841 [0.832, 0.848] | 0.846 [0.830, 0.858] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.698 / 0.734 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n1024 | products2 |  | 0.958 [0.952, 0.969] | 0.967 [0.947, 0.984] | 0.671 / 0.704 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n1024 | products4 |  | 0.957 [0.950, 0.962] | 0.957 [0.949, 0.971] | 0.667 / 0.703 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n1024 | deferred_columns |  | 1.694 [1.682, 1.707] | 1.678 [1.655, 1.723] | 1.180 / 1.228 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_full_n1024 | incumbent | yes | 0.953 [0.950, 0.960] | 0.951 [0.943, 0.968] | 0.665 / 0.698 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 44.114 / 46.159 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n65536 | products2 |  | 0.955 [0.950, 0.960] | 0.952 [0.943, 0.959] | 42.166 / 43.641 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n65536 | products4 |  | 0.955 [0.949, 0.958] | 0.950 [0.944, 0.966] | 42.094 / 43.890 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_full_n65536 | deferred_columns |  | 1.676 [1.669, 1.685] | 1.667 [1.645, 1.679] | 74.032 / 76.317 | 2.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l2_full_n65536 | incumbent | yes | 0.953 [0.947, 0.957] | 0.949 [0.942, 0.960] | 42.069 / 43.507 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.348 / 0.361 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n16 | products2 |  | 1.022 [1.013, 1.034] | 1.029 [0.980, 1.115] | 0.356 / 0.378 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_signed16_n16 | products4 |  | 1.056 [1.049, 1.065] | 1.070 [1.044, 1.098] | 0.367 / 0.385 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_signed16_n16 | deferred_columns |  | 2.460 [2.441, 2.473] | 2.452 [2.398, 2.564] | 0.855 / 0.888 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_signed16_n16 | incumbent | yes | 0.809 [0.804, 0.816] | 0.825 [0.804, 0.996] | 0.281 / 0.316 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 22.174 / 25.431 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | products2 |  | 1.009 [1.000, 1.018] | 1.009 [0.896, 1.051] | 22.395 / 26.560 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | products4 |  | 1.008 [0.996, 1.015] | 1.012 [0.785, 1.053] | 22.343 / 26.044 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | deferred_columns |  | 2.446 [2.429, 2.472] | 2.378 [2.070, 2.462] | 54.354 / 61.449 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | incumbent | yes | 0.805 [0.795, 0.812] | 0.803 [0.578, 0.825] | 17.781 / 20.422 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1426.068 / 1607.857 | 9.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | products2 |  | 1.011 [1.002, 1.017] | 1.039 [0.966, 1.154] | 1439.536 / 1725.092 | 9.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | products4 |  | 1.012 [1.005, 1.018] | 1.032 [0.999, 1.203] | 1443.448 / 1620.752 | 9.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | deferred_columns |  | 2.460 [2.449, 2.478] | 2.437 [2.236, 2.593] | 3512.771 / 3891.376 | 9.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | incumbent | yes | 0.814 [0.805, 0.821] | 0.813 [0.741, 0.908] | 1162.344 / 1273.426 | 9.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.348 / 0.378 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n16 | products2 |  | 1.025 [1.016, 1.031] | 1.021 [0.890, 1.044] | 0.357 / 0.378 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_full_n16 | products4 |  | 1.052 [1.048, 1.060] | 1.056 [0.711, 1.080] | 0.367 / 0.389 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_full_n16 | deferred_columns |  | 2.467 [2.448, 2.486] | 2.441 [1.867, 2.499] | 0.861 / 0.944 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_full_n16 | incumbent | yes | 0.809 [0.804, 0.814] | 0.821 [0.789, 0.915] | 0.281 / 0.333 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 22.212 / 23.358 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n1024 | products2 |  | 1.012 [1.005, 1.019] | 1.027 [1.008, 1.176] | 22.540 / 31.305 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_full_n1024 | products4 |  | 1.009 [1.003, 1.023] | 1.028 [1.008, 1.518] | 22.484 / 31.273 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_full_n1024 | deferred_columns |  | 2.456 [2.445, 2.471] | 2.474 [2.430, 3.587] | 54.662 / 63.082 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_full_n1024 | incumbent | yes | 0.810 [0.804, 0.821] | 0.833 [0.812, 0.901] | 18.058 / 20.996 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1421.390 / 1463.006 | 9.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l9_full_n65536 | products2 |  | 1.007 [1.002, 1.012] | 1.007 [0.990, 1.032] | 1432.156 / 1475.846 | 9.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_full_n65536 | products4 |  | 1.009 [1.003, 1.015] | 1.009 [0.994, 1.021] | 1432.927 / 1476.639 | 9.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l9_full_n65536 | deferred_columns |  | 2.462 [2.453, 2.471] | 2.436 [2.392, 2.473] | 3498.943 / 3580.060 | 9.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l9_full_n65536 | incumbent | yes | 0.811 [0.803, 0.814] | 0.818 [0.798, 0.825] | 1150.292 / 1194.824 | 9.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.054 / 0.056 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n16 | products2 |  | 1.090 [1.084, 1.102] | 1.092 [1.075, 1.108] | 0.059 / 0.061 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n16 | products4 |  | 1.118 [1.110, 1.126] | 1.116 [1.102, 1.136] | 0.060 / 0.063 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n16 | deferred_columns |  | 1.244 [1.232, 1.251] | 1.236 [1.219, 1.244] | 0.067 / 0.070 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n16 | incumbent |  | 1.085 [1.075, 1.092] | 1.086 [1.071, 1.112] | 0.058 / 0.062 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n16 | native128x2 | yes | 0.916 [0.907, 0.923] | 0.907 [0.902, 0.929] | 0.049 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n16 | native128x2_acc2 |  | 0.913 [0.903, 0.917] | 0.908 [0.900, 0.924] | 0.049 / 0.051 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.201 / 3.363 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | products2 |  | 1.073 [1.064, 1.082] | 1.070 [1.049, 1.083] | 3.423 / 3.626 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | products4 |  | 1.119 [1.107, 1.126] | 1.112 [1.092, 1.125] | 3.565 / 3.755 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | deferred_columns |  | 1.298 [1.278, 1.312] | 1.282 [1.260, 1.296] | 4.120 / 4.350 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | incumbent |  | 1.341 [1.324, 1.347] | 1.318 [1.296, 1.332] | 4.258 / 4.460 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | native128x2 | yes | 0.907 [0.899, 0.916] | 0.913 [0.897, 0.929] | 2.904 / 3.083 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | native128x2_acc2 |  | 0.932 [0.926, 0.940] | 0.932 [0.918, 0.951] | 2.981 / 3.154 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 208.212 / 219.636 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | products2 |  | 1.075 [1.067, 1.084] | 1.071 [1.047, 1.123] | 223.458 / 235.472 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | products4 |  | 1.093 [1.084, 1.103] | 1.092 [1.042, 1.106] | 227.996 / 237.169 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | deferred_columns |  | 1.290 [1.280, 1.307] | 1.273 [1.252, 1.331] | 269.311 / 282.575 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | incumbent |  | 1.311 [1.302, 1.320] | 1.301 [1.278, 1.485] | 273.059 / 286.086 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | native128x2 | yes | 0.923 [0.913, 0.929] | 0.919 [0.895, 0.987] | 192.322 / 200.200 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | native128x2_acc2 |  | 0.913 [0.908, 0.925] | 0.918 [0.891, 0.946] | 190.988 / 199.969 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.054 / 0.057 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n16 | products2 |  | 1.072 [1.065, 1.100] | 1.073 [1.057, 1.097] | 0.058 / 0.061 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | products4 |  | 1.113 [1.110, 1.125] | 1.114 [1.078, 1.150] | 0.060 / 0.063 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | deferred_columns |  | 1.273 [1.247, 1.276] | 1.259 [1.199, 1.278] | 0.068 / 0.071 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | incumbent |  | 1.086 [1.076, 1.091] | 1.072 [1.058, 1.096] | 0.058 / 0.061 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | native128x2 | yes | 0.920 [0.915, 0.925] | 0.911 [0.894, 0.933] | 0.049 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n16 | native128x2_acc2 |  | 0.918 [0.911, 0.925] | 0.922 [0.905, 0.978] | 0.049 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.208 / 3.342 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n1024 | products2 |  | 1.077 [1.058, 1.082] | 1.080 [1.034, 1.111] | 3.431 / 3.553 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n1024 | products4 |  | 1.114 [1.102, 1.128] | 1.109 [1.081, 1.133] | 3.568 / 3.718 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n1024 | deferred_columns |  | 1.292 [1.278, 1.301] | 1.272 [1.237, 1.308] | 4.127 / 4.268 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n1024 | incumbent |  | 1.329 [1.314, 1.341] | 1.324 [1.293, 1.351] | 4.259 / 4.458 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n1024 | native128x2 | yes | 0.915 [0.901, 0.923] | 0.918 [0.883, 0.945] | 2.918 / 3.061 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n1024 | native128x2_acc2 |  | 0.938 [0.926, 0.944] | 0.926 [0.908, 0.947] | 2.991 / 3.130 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 210.161 / 223.177 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n65536 | products2 |  | 1.076 [1.069, 1.093] | 1.080 [1.058, 1.103] | 227.417 / 242.367 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n65536 | products4 |  | 1.098 [1.086, 1.106] | 1.091 [1.073, 1.109] | 230.423 / 246.113 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n65536 | deferred_columns |  | 1.295 [1.281, 1.311] | 1.291 [1.263, 1.317] | 271.732 / 288.073 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n65536 | incumbent |  | 1.312 [1.295, 1.328] | 1.304 [1.282, 1.337] | 275.328 / 296.769 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n65536 | native128x2 | yes | 0.924 [0.907, 0.933] | 0.923 [0.901, 0.933] | 193.129 / 211.491 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n65536 | native128x2_acc2 |  | 0.914 [0.906, 0.928] | 0.915 [0.887, 0.928] | 192.324 / 205.321 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.949 / 3.086 | 0.000 | 11 / 168 | pass |
| aarch64 | opt_projection | q100_l2_n16 | public_divisor |  | 0.330 [0.308, 0.339] | 0.331 [0.322, 0.349] | 0.972 / 1.076 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n16 | horner_prepared | yes | 0.042 [0.041, 0.043] | 0.044 [0.041, 0.045] | 0.124 / 0.135 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n16 | horner_one_shot | yes | 0.095 [0.094, 0.097] | 0.096 [0.094, 0.098] | 0.282 / 0.299 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 581.716 / 606.665 | 0.031 | 522 / 8344 | pass |
| aarch64 | opt_projection | q100_l2_n1024 | public_divisor |  | 0.114 [0.112, 0.116] | 0.116 [0.114, 0.119] | 66.299 / 70.524 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n1024 | horner_prepared | yes | 0.013 [0.013, 0.014] | 0.014 [0.013, 0.015] | 7.781 / 8.426 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l2_n1024 | horner_one_shot | yes | 0.014 [0.014, 0.014] | 0.014 [0.014, 0.015] | 7.987 / 8.554 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.318 / 0.330 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n16 | public_divisor |  | 2.422 [2.365, 2.977] | 2.413 [2.342, 2.942] | 0.772 / 0.793 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_projection | q128_l2_n16 | horner_prepared | yes | 0.396 [0.380, 0.480] | 0.395 [0.385, 0.483] | 0.124 / 0.132 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n16 | horner_one_shot |  | 0.868 [0.838, 1.062] | 0.883 [0.848, 1.069] | 0.275 / 0.287 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_projection | q128_l2_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 23.171 / 24.265 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n1024 | public_divisor |  | 2.102 [2.073, 2.123] | 2.073 [2.033, 2.154] | 48.648 / 50.485 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_projection | q128_l2_n1024 | horner_prepared | yes | 0.333 [0.328, 0.337] | 0.335 [0.326, 0.343] | 7.712 / 8.246 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n1024 | horner_one_shot | yes | 0.343 [0.336, 0.345] | 0.346 [0.336, 0.352] | 7.923 / 8.430 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 5.785 / 7.858 | 0.001 | 11 / 328 | pass |
| aarch64 | opt_projection | q100_l4_n16 | public_divisor |  | 0.241 [0.209, 0.245] | 0.211 [0.182, 0.241] | 1.374 / 1.490 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n16 | horner_prepared | yes | 0.082 [0.073, 0.085] | 0.072 [0.064, 0.083] | 0.470 / 0.511 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n16 | horner_one_shot | yes | 0.113 [0.099, 0.115] | 0.099 [0.084, 0.110] | 0.644 / 0.704 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1385.859 / 1438.848 | 0.047 | 547 / 17480 | pass |
| aarch64 | opt_projection | q100_l4_n1024 | public_divisor |  | 0.061 [0.061, 0.062] | 0.063 [0.061, 0.064] | 84.568 / 91.335 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n1024 | horner_prepared | yes | 0.022 [0.021, 0.022] | 0.022 [0.022, 0.023] | 29.833 / 32.241 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l4_n1024 | horner_one_shot | yes | 0.022 [0.021, 0.022] | 0.023 [0.022, 0.023] | 30.052 / 32.609 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.044 / 1.176 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n16 | public_divisor |  | 1.234 [1.080, 1.263] | 1.190 [1.056, 1.224] | 1.256 / 1.319 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_projection | q128_l4_n16 | horner_prepared | yes | 0.460 [0.403, 0.473] | 0.451 [0.400, 0.465] | 0.470 / 0.506 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n16 | horner_one_shot | yes | 0.618 [0.543, 0.647] | 0.595 [0.523, 0.613] | 0.641 / 0.664 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 84.158 / 89.305 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n1024 | public_divisor |  | 0.957 [0.941, 0.965] | 0.925 [0.901, 0.948] | 79.986 / 83.684 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n1024 | horner_prepared | yes | 0.357 [0.352, 0.361] | 0.363 [0.344, 0.369] | 29.972 / 32.120 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l4_n1024 | horner_one_shot | yes | 0.359 [0.354, 0.363] | 0.356 [0.350, 0.368] | 30.218 / 32.183 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 28.552 / 36.203 | 0.001 | 11 / 728 | pass |
| aarch64 | opt_projection | q100_l9_n16 | public_divisor |  | 0.100 [0.097, 0.107] | 0.086 [0.082, 0.092] | 2.879 / 3.096 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n16 | horner_prepared | yes | 0.053 [0.051, 0.056] | 0.046 [0.042, 0.050] | 1.506 / 1.630 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n16 | horner_one_shot | yes | 0.060 [0.059, 0.066] | 0.052 [0.050, 0.057] | 1.758 / 1.893 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3395.938 / 3630.382 | 0.086 | 544 / 39104 | pass |
| aarch64 | opt_projection | q100_l9_n1024 | public_divisor |  | 0.053 [0.052, 0.053] | 0.055 [0.052, 0.058] | 180.230 / 197.354 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n1024 | horner_prepared | yes | 0.028 [0.028, 0.028] | 0.029 [0.028, 0.030] | 95.750 / 105.410 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q100_l9_n1024 | horner_one_shot | yes | 0.028 [0.028, 0.028] | 0.029 [0.028, 0.030] | 96.209 / 106.967 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.925 / 3.144 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n16 | public_divisor |  | 0.946 [0.892, 1.015] | 0.946 [0.889, 1.033] | 2.763 / 2.894 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_projection | q128_l9_n16 | horner_prepared | yes | 0.513 [0.489, 0.553] | 0.520 [0.495, 0.562] | 1.513 / 1.615 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n16 | horner_one_shot | yes | 0.597 [0.564, 0.643] | 0.595 [0.575, 0.649] | 1.747 / 1.886 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n1024 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 235.514 / 247.288 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n1024 | public_divisor |  | 0.732 [0.723, 0.747] | 0.747 [0.730, 0.751] | 173.147 / 182.191 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n1024 | horner_prepared | yes | 0.406 [0.401, 0.413] | 0.414 [0.404, 0.420] | 95.779 / 102.145 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l9_n1024 | horner_one_shot | yes | 0.408 [0.402, 0.413] | 0.417 [0.406, 0.421] | 96.074 / 102.864 | 0.086 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q100_l2 | horner | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.158 / 0.165 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q128_l2 | horner | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.149 / 0.163 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q100_l4 | horner | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.178 / 0.184 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q128_l4 | horner | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.170 / 0.177 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q100_l9 | horner | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.229 / 0.241 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection_setup | q128_l9 | horner | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.223 / 0.236 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n16 | generic |  | 2.177 [2.172, 2.213] | 2.144 [2.111, 2.172] | 0.821 / 0.856 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l2_n16 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.377 / 0.398 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n16 | prepared_reciprocal | yes | 0.264 [0.263, 0.266] | 0.268 [0.263, 0.274] | 0.099 / 0.107 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n16 | prepare_batch |  | 0.334 [0.332, 0.336] | 0.339 [0.328, 0.348] | 0.126 / 0.134 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n1024 | generic |  | 2.195 [2.188, 2.215] | 2.179 [2.157, 2.207] | 52.590 / 54.619 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l2_n1024 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 23.900 / 25.095 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n1024 | prepared_reciprocal | yes | 0.264 [0.262, 0.267] | 0.269 [0.265, 0.277] | 6.334 / 6.826 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l2_n1024 | prepare_batch |  | 0.274 [0.273, 0.278] | 0.279 [0.275, 0.288] | 6.594 / 6.978 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n16 | generic |  | 2.400 [2.386, 2.414] | 2.411 [2.377, 2.433] | 1.522 / 1.579 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l4_n16 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.633 / 0.664 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n16 | prepared_reciprocal | yes | 0.394 [0.392, 0.398] | 0.405 [0.401, 0.409] | 0.250 / 0.267 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n16 | prepare_batch |  | 0.432 [0.423, 0.435] | 0.445 [0.435, 0.453] | 0.273 / 0.292 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n1024 | generic |  | 2.402 [2.393, 2.417] | 2.397 [2.381, 2.418] | 97.213 / 101.393 | 0.094 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l4_n1024 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 40.516 / 42.398 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n1024 | prepared_reciprocal | yes | 0.395 [0.393, 0.399] | 0.405 [0.398, 0.412] | 16.020 / 17.003 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l4_n1024 | prepare_batch |  | 0.399 [0.397, 0.403] | 0.410 [0.403, 0.418] | 16.208 / 17.589 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n16 | generic |  | 3.367 [3.350, 3.384] | 3.329 [3.310, 3.353] | 4.336 / 4.476 | 0.003 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l9_n16 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.286 / 1.345 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n16 | prepared_reciprocal | yes | 0.545 [0.541, 0.549] | 0.551 [0.546, 0.567] | 0.701 / 0.759 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n16 | prepare_batch |  | 0.571 [0.566, 0.575] | 0.574 [0.569, 0.590] | 0.736 / 0.773 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n1024 | generic |  | 3.370 [3.353, 3.390] | 3.345 [3.327, 3.368] | 277.192 / 286.910 | 0.211 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l9_n1024 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 82.085 / 85.793 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n1024 | prepared_reciprocal | yes | 0.544 [0.539, 0.548] | 0.549 [0.544, 0.558] | 44.727 / 47.031 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l9_n1024 | prepare_batch |  | 0.547 [0.545, 0.555] | 0.557 [0.552, 0.567] | 45.282 / 47.715 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n16 | generic |  | 5.530 [5.516, 5.554] | 5.492 [5.384, 5.559] | 27.137 / 28.206 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l32_n16 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.908 / 5.159 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n16 | prepared_reciprocal | yes | 0.819 [0.814, 0.826] | 0.820 [0.814, 0.838] | 4.028 / 4.241 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n16 | prepare_batch |  | 0.825 [0.817, 0.830] | 0.823 [0.818, 0.835] | 4.050 / 4.245 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n1024 | generic |  | 5.568 [5.531, 5.601] | 5.477 [5.422, 5.555] | 1734.922 / 1813.154 | 0.750 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l32_n1024 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 312.982 / 330.884 | 0.750 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n1024 | prepared_reciprocal | yes | 0.822 [0.815, 0.829] | 0.822 [0.791, 0.833] | 257.279 / 274.122 | 0.750 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l32_n1024 | prepare_batch |  | 0.823 [0.817, 0.833] | 0.821 [0.807, 0.832] | 257.746 / 273.303 | 0.750 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n16 | generic |  | 9.233 [9.201, 9.302] | 9.169 [9.067, 9.231] | 87.395 / 91.789 | 0.023 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l64_n16 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 9.457 / 10.078 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n16 | prepared_reciprocal | yes | 0.852 [0.847, 0.861] | 0.856 [0.845, 0.875] | 8.075 / 8.715 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n16 | prepare_batch |  | 0.856 [0.845, 0.861] | 0.856 [0.848, 0.873] | 8.088 / 8.658 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n1024 | generic |  | 9.146 [9.099, 9.221] | 9.046 [9.009, 9.212] | 5612.159 / 5851.184 | 1.500 | 0 / 0 | regression |
| aarch64 | opt_divrem64 | l64_n1024 | public_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 612.029 / 647.838 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n1024 | prepared_reciprocal | yes | 0.846 [0.842, 0.856] | 0.855 [0.850, 0.863] | 520.651 / 558.854 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_divrem64 | l64_n1024 | prepare_batch |  | 0.847 [0.841, 0.856] | 0.861 [0.854, 0.869] | 520.784 / 551.922 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l1_n16 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.005 / 0.005 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l1_n16 | word_carry |  | 1.044 [1.035, 1.048] | 1.036 [1.028, 1.049] | 0.005 / 0.005 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_uint_add | l1_n16 | native_batch |  | 1.043 [1.032, 1.047] | 1.036 [1.032, 1.046] | 0.005 / 0.005 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_uint_add | l1_n1024 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.212 / 0.229 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l1_n1024 | word_carry |  | 1.001 [0.985, 1.004] | 1.012 [0.993, 1.094] | 0.212 / 0.228 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_add | l1_n1024 | native_batch |  | 0.997 [0.979, 1.002] | 1.003 [0.927, 1.025] | 0.211 / 0.224 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_add | l2_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.016 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l2_n16 | word_carry |  | 1.023 [1.010, 1.032] | 1.032 [0.988, 1.138] | 0.012 / 0.013 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_uint_add | l2_n16 | native_batch | yes | 0.734 [0.719, 0.739] | 0.736 [0.708, 0.746] | 0.009 / 0.009 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l2_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.665 / 0.699 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l2_n1024 | word_carry |  | 1.000 [0.995, 1.007] | 1.005 [0.995, 1.024] | 0.664 / 0.699 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_add | l2_n1024 | native_batch | yes | 0.690 [0.684, 0.694] | 0.694 [0.682, 0.710] | 0.460 / 0.485 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l4_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l4_n16 | word_carry |  | 0.768 [0.764, 0.775] | 0.779 [0.766, 0.786] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l4_n16 | native_batch | yes | 0.778 [0.772, 0.782] | 0.777 [0.769, 0.787] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l4_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.955 / 1.002 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l4_n1024 | word_carry |  | 0.745 [0.737, 0.780] | 0.747 [0.730, 0.789] | 0.708 / 0.785 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l4_n1024 | native_batch | yes | 0.743 [0.736, 0.757] | 0.741 [0.712, 0.768] | 0.707 / 0.770 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l9_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.060 / 0.065 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l9_n16 | word_carry |  | 0.837 [0.819, 0.902] | 0.859 [0.853, 0.922] | 0.051 / 0.055 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l9_n16 | native_batch | yes | 0.765 [0.753, 0.816] | 0.771 [0.760, 0.816] | 0.046 / 0.050 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l9_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.593 / 3.854 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l9_n1024 | word_carry |  | 0.894 [0.890, 0.907] | 0.901 [0.876, 0.910] | 3.229 / 3.459 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l9_n1024 | native_batch | yes | 0.863 [0.852, 0.873] | 0.865 [0.843, 0.870] | 3.102 / 3.295 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l1_n16 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.005 / 0.005 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l1_n16 | word_carry |  | 1.039 [1.032, 1.052] | 1.039 [1.023, 1.054] | 0.005 / 0.005 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_uint_sub | l1_n16 | native_batch |  | 1.038 [1.032, 1.050] | 1.042 [1.018, 1.048] | 0.005 / 0.005 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_uint_sub | l1_n1024 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.212 / 0.224 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l1_n1024 | word_carry |  | 1.001 [0.993, 1.009] | 1.001 [0.991, 1.015] | 0.212 / 0.225 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l1_n1024 | native_batch |  | 0.998 [0.991, 1.005] | 1.003 [0.990, 1.015] | 0.212 / 0.225 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l2_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l2_n16 | word_carry |  | 1.032 [1.023, 1.038] | 1.022 [1.005, 1.038] | 0.012 / 0.013 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_uint_sub | l2_n16 | native_batch | yes | 0.728 [0.724, 0.734] | 0.740 [0.716, 0.752] | 0.008 / 0.009 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l2_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.661 / 0.694 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l2_n1024 | word_carry |  | 1.003 [0.997, 1.007] | 1.006 [0.987, 1.016] | 0.663 / 0.690 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l2_n1024 | native_batch | yes | 0.692 [0.685, 0.695] | 0.698 [0.685, 0.725] | 0.457 / 0.481 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l4_n16 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.022 / 0.024 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l4_n16 | word_carry |  | 0.996 [0.992, 1.002] | 0.995 [0.987, 1.006] | 0.022 / 0.023 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l4_n16 | native_batch |  | 0.995 [0.990, 1.001] | 0.991 [0.981, 1.010] | 0.022 / 0.024 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l4_n1024 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.342 / 1.409 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l4_n1024 | word_carry |  | 0.995 [0.985, 1.013] | 0.990 [0.979, 1.009] | 1.340 / 1.381 | 0.094 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l4_n1024 | native_batch |  | 1.005 [0.998, 1.010] | 1.003 [0.992, 1.010] | 1.350 / 1.409 | 0.094 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l9_n16 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.062 / 0.066 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l9_n16 | word_carry |  | 1.019 [1.000, 1.037] | 1.023 [0.992, 1.048] | 0.064 / 0.066 | 0.003 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l9_n16 | native_batch |  | 0.974 [0.958, 0.997] | 0.977 [0.969, 1.023] | 0.061 / 0.064 | 0.003 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l9_n1024 | circuit_z | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.978 / 4.189 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l9_n1024 | word_carry |  | 0.987 [0.976, 0.990] | 0.981 [0.968, 0.992] | 3.908 / 4.101 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l9_n1024 | native_batch |  | 0.979 [0.973, 0.989] | 0.985 [0.964, 1.021] | 3.902 / 4.058 | 0.211 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_checked_add | l1_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.019 / 0.020 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n16 | word_option |  | 0.465 [0.457, 0.468] | 0.479 [0.462, 0.483] | 0.009 / 0.010 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n16 | native_option |  | 0.450 [0.446, 0.452] | 0.458 [0.448, 0.462] | 0.009 / 0.009 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n16 | limb_option | yes | 0.451 [0.447, 0.454] | 0.461 [0.452, 0.468] | 0.009 / 0.009 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n1024 | vendor_option | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.489 / 0.517 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n1024 | word_option |  | 1.003 [0.995, 1.010] | 1.004 [0.994, 1.015] | 0.490 / 0.517 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_checked_add | l1_n1024 | native_option |  | 0.965 [0.960, 0.980] | 0.976 [0.960, 0.984] | 0.475 / 0.498 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n1024 | limb_option |  | 0.982 [0.973, 0.986] | 0.976 [0.968, 0.988] | 0.479 / 0.505 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l2_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.014 / 0.018 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l2_n16 | word_option |  | 1.591 [1.490, 1.605] | 1.374 [1.294, 1.436] | 0.022 / 0.024 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_add | l2_n16 | native_option |  | 1.152 [1.072, 1.163] | 0.944 [0.928, 1.032] | 0.016 / 0.017 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_add | l2_n16 | limb_option | yes | 0.893 [0.829, 0.901] | 0.730 [0.720, 0.801] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l2_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.727 / 0.771 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l2_n1024 | word_option |  | 1.702 [1.678, 1.718] | 1.690 [1.665, 1.725] | 1.238 / 1.291 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_add | l2_n1024 | native_option |  | 1.254 [1.250, 1.261] | 1.260 [1.247, 1.274] | 0.913 / 0.963 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_add | l2_n1024 | limb_option | yes | 0.918 [0.914, 0.924] | 0.923 [0.916, 0.934] | 0.666 / 0.715 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.045 / 0.048 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n16 | word_option |  | 0.638 [0.626, 0.674] | 0.668 [0.619, 0.678] | 0.030 / 0.030 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n16 | native_option |  | 0.545 [0.513, 0.561] | 0.550 [0.518, 0.580] | 0.024 / 0.026 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n16 | limb_option | yes | 0.350 [0.323, 0.358] | 0.357 [0.322, 0.368] | 0.015 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.617 / 3.929 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n1024 | word_option |  | 0.415 [0.411, 0.433] | 0.422 [0.411, 0.435] | 1.506 / 1.645 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n1024 | native_option |  | 0.406 [0.401, 0.422] | 0.412 [0.403, 0.433] | 1.469 / 1.584 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l4_n1024 | limb_option | yes | 0.243 [0.239, 0.253] | 0.249 [0.240, 0.259] | 0.881 / 0.949 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.084 / 0.090 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n16 | word_option |  | 0.725 [0.698, 0.736] | 0.722 [0.710, 0.736] | 0.061 / 0.063 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n16 | native_option |  | 0.702 [0.686, 0.713] | 0.715 [0.705, 0.725] | 0.059 / 0.063 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n16 | limb_option | yes | 0.699 [0.695, 0.729] | 0.724 [0.699, 0.736] | 0.059 / 0.064 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 7.181 / 7.557 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n1024 | word_option |  | 0.565 [0.552, 0.580] | 0.573 [0.557, 0.626] | 4.067 / 4.344 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n1024 | native_option |  | 0.563 [0.551, 0.578] | 0.568 [0.551, 0.653] | 4.054 / 4.328 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l9_n1024 | limb_option | yes | 0.571 [0.562, 0.590] | 0.590 [0.560, 0.610] | 4.137 / 4.407 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.019 / 0.020 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n16 | word_option |  | 0.488 [0.484, 0.494] | 0.495 [0.487, 0.513] | 0.009 / 0.010 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n16 | native_option | yes | 0.458 [0.454, 0.464] | 0.468 [0.453, 0.480] | 0.009 / 0.009 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n16 | limb_option |  | 0.460 [0.458, 0.463] | 0.466 [0.448, 0.481] | 0.009 / 0.009 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n1024 | vendor_option | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.465 / 0.497 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n1024 | word_option |  | 1.065 [1.060, 1.075] | 1.065 [1.044, 1.106] | 0.497 / 0.533 | 0.023 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_sub | l1_n1024 | native_option |  | 0.967 [0.960, 0.973] | 0.973 [0.958, 1.002] | 0.450 / 0.481 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l1_n1024 | limb_option |  | 0.966 [0.954, 0.973] | 0.970 [0.948, 0.992] | 0.450 / 0.481 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l2_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.026 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l2_n16 | word_option |  | 1.029 [0.984, 1.037] | 0.905 [0.889, 0.987] | 0.022 / 0.024 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_checked_sub | l2_n16 | native_option | yes | 0.584 [0.552, 0.587] | 0.510 [0.504, 0.551] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l2_n16 | limb_option |  | 0.946 [0.892, 0.950] | 0.826 [0.807, 0.898] | 0.020 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l2_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.193 / 1.252 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l2_n1024 | word_option |  | 1.017 [1.012, 1.023] | 1.020 [1.009, 1.033] | 1.214 / 1.266 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_sub | l2_n1024 | native_option | yes | 0.559 [0.555, 0.562] | 0.561 [0.557, 0.573] | 0.666 / 0.706 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l2_n1024 | limb_option |  | 0.969 [0.962, 0.975] | 0.968 [0.959, 0.980] | 1.158 / 1.206 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.043 / 0.046 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n16 | word_option |  | 0.663 [0.648, 0.676] | 0.677 [0.667, 0.695] | 0.029 / 0.031 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n16 | native_option | yes | 0.580 [0.572, 0.584] | 0.581 [0.575, 0.598] | 0.025 / 0.027 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n16 | limb_option |  | 0.646 [0.635, 0.656] | 0.653 [0.641, 0.669] | 0.028 / 0.030 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.383 / 3.594 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n1024 | word_option |  | 0.531 [0.518, 0.546] | 0.546 [0.531, 0.561] | 1.808 / 1.930 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n1024 | native_option | yes | 0.531 [0.523, 0.544] | 0.545 [0.535, 0.562] | 1.812 / 1.941 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l4_n1024 | limb_option |  | 0.506 [0.493, 0.508] | 0.510 [0.498, 0.524] | 1.698 / 1.842 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.093 / 0.100 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n16 | word_option |  | 0.715 [0.709, 0.733] | 0.720 [0.702, 0.732] | 0.067 / 0.071 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n16 | native_option | yes | 0.710 [0.696, 0.732] | 0.727 [0.701, 0.733] | 0.066 / 0.071 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n16 | limb_option |  | 0.799 [0.792, 0.818] | 0.799 [0.779, 0.830] | 0.075 / 0.079 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 7.376 / 7.700 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n1024 | word_option |  | 0.561 [0.556, 0.569] | 0.571 [0.561, 0.585] | 4.149 / 4.394 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n1024 | native_option | yes | 0.560 [0.556, 0.567] | 0.571 [0.560, 0.583] | 4.137 / 4.397 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l9_n1024 | limb_option |  | 0.636 [0.632, 0.641] | 0.643 [0.638, 0.657] | 4.709 / 4.975 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l1_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.022 / 0.022 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l1_n16 | word_option | yes | 0.562 [0.551, 0.590] | 0.569 [0.563, 0.597] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l1_n16 | native_option |  | 0.545 [0.532, 0.567] | 0.550 [0.543, 0.572] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l1_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.224 / 1.403 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l1_n1024 | word_option | yes | 0.577 [0.563, 0.583] | 0.540 [0.484, 0.563] | 0.705 / 0.747 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l1_n1024 | native_option |  | 0.574 [0.556, 0.581] | 0.540 [0.478, 0.562] | 0.700 / 0.746 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l2_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.022 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l2_n16 | word_option | yes | 0.857 [0.846, 0.864] | 0.864 [0.840, 0.873] | 0.018 / 0.019 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l2_n16 | native_option |  | 0.629 [0.622, 0.636] | 0.639 [0.618, 0.648] | 0.013 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l2_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.629 / 1.724 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l2_n1024 | word_option | yes | 0.466 [0.459, 0.477] | 0.461 [0.454, 0.485] | 0.762 / 0.818 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l2_n1024 | native_option |  | 0.467 [0.460, 0.485] | 0.474 [0.460, 0.486] | 0.763 / 0.811 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l4_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.048 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l4_n16 | word_option | yes | 0.517 [0.473, 0.539] | 0.526 [0.472, 0.541] | 0.024 / 0.025 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l4_n16 | native_option |  | 0.429 [0.392, 0.445] | 0.436 [0.394, 0.443] | 0.020 / 0.022 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l4_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.588 / 3.793 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l4_n1024 | word_option | yes | 0.335 [0.330, 0.339] | 0.341 [0.335, 0.347] | 1.195 / 1.290 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l4_n1024 | native_option |  | 0.333 [0.327, 0.338] | 0.338 [0.329, 0.348] | 1.186 / 1.304 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l9_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.094 / 0.100 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l9_n16 | word_option | yes | 0.576 [0.570, 0.584] | 0.584 [0.573, 0.599] | 0.054 / 0.058 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l9_n16 | native_option |  | 0.566 [0.559, 0.575] | 0.581 [0.569, 0.589] | 0.053 / 0.057 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l9_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 7.411 / 14.471 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l9_n1024 | word_option | yes | 0.456 [0.446, 0.460] | 0.469 [0.450, 0.517] | 3.384 / 5.657 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_int_checked_add | l9_n1024 | native_option |  | 0.454 [0.449, 0.461] | 0.472 [0.460, 0.571] | 3.377 / 5.558 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.018 / 0.018 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n16 | word_option | yes | 0.695 [0.683, 0.701] | 0.697 [0.686, 0.709] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n16 | native_option |  | 0.667 [0.657, 0.674] | 0.668 [0.661, 0.681] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.078 / 1.457 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n1024 | word_option | yes | 0.673 [0.623, 0.685] | 0.533 [0.493, 0.592] | 0.707 / 0.757 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n1024 | native_option |  | 0.671 [0.622, 0.684] | 0.540 [0.489, 0.754] | 0.706 / 0.755 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.022 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n16 | word_option | yes | 0.857 [0.853, 0.865] | 0.858 [0.851, 0.883] | 0.018 / 0.019 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n16 | native_option |  | 0.629 [0.622, 0.635] | 0.635 [0.623, 0.648] | 0.013 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.415 / 1.679 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n1024 | word_option | yes | 0.531 [0.514, 0.579] | 0.485 [0.448, 0.554] | 0.764 / 0.818 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n1024 | native_option |  | 0.514 [0.501, 0.564] | 0.470 [0.448, 0.544] | 0.750 / 0.798 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l4_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.052 / 0.057 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l4_n16 | word_option | yes | 0.532 [0.478, 0.546] | 0.534 [0.482, 0.545] | 0.027 / 0.030 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l4_n16 | native_option |  | 0.453 [0.446, 0.456] | 0.457 [0.446, 0.468] | 0.023 / 0.025 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l4_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.018 / 4.373 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l4_n1024 | word_option | yes | 0.346 [0.330, 0.351] | 0.348 [0.342, 0.362] | 1.382 / 1.488 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l4_n1024 | native_option |  | 0.342 [0.334, 0.354] | 0.351 [0.342, 0.369] | 1.382 / 1.493 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l9_n16 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.114 / 0.121 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l9_n16 | word_option | yes | 0.585 [0.561, 0.587] | 0.589 [0.577, 0.602] | 0.066 / 0.070 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l9_n16 | native_option |  | 0.567 [0.542, 0.577] | 0.575 [0.557, 0.586] | 0.065 / 0.069 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l9_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.861 / 9.287 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l9_n1024 | word_option | yes | 0.450 [0.447, 0.465] | 0.463 [0.452, 0.469] | 4.023 / 4.264 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l9_n1024 | native_option |  | 0.449 [0.446, 0.461] | 0.461 [0.450, 0.468] | 3.991 / 4.280 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a1_b1_n16 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.006 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a1_b1_n16 | schoolbook |  | 1.175 [1.162, 1.189] | 1.172 [1.161, 1.182] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a1_b1_n1024 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.335 / 0.351 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a1_b1_n1024 | schoolbook |  | 1.152 [1.144, 1.156] | 1.153 [1.141, 1.160] | 0.384 / 0.402 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a2_b2_n16 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.023 / 0.024 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a2_b2_n16 | schoolbook |  | 1.010 [1.003, 1.018] | 1.005 [0.997, 1.029] | 0.023 / 0.024 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_exact_product | a2_b2_n1024 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.343 / 1.411 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a2_b2_n1024 | schoolbook |  | 1.007 [0.996, 1.011] | 1.006 [0.999, 1.024] | 1.345 / 1.420 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | opt_exact_product | a4_b4_n16 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.107 / 0.114 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a4_b4_n16 | schoolbook |  | 1.019 [1.010, 1.027] | 1.018 [1.003, 1.027] | 0.109 / 0.116 | 0.002 | 0 / 0 | inconclusive |
| aarch64 | opt_exact_product | a4_b4_n1024 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6.428 / 6.937 | 0.125 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a4_b4_n1024 | schoolbook |  | 1.025 [1.015, 1.032] | 1.028 [1.020, 1.052] | 6.590 / 7.105 | 0.125 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a9_b9_n16 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.795 / 0.845 | 0.004 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a9_b9_n16 | schoolbook |  | 1.243 [1.202, 1.265] | 1.283 [1.221, 1.323] | 0.977 / 1.071 | 0.004 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a9_b9_n1024 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 50.940 / 54.219 | 0.281 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a9_b9_n1024 | schoolbook |  | 1.224 [1.197, 1.282] | 1.309 [1.247, 1.320] | 63.678 / 68.727 | 0.281 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a2_b9_n16 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.116 / 0.123 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a2_b9_n16 | schoolbook |  | 1.005 [0.996, 1.008] | 0.996 [0.989, 1.003] | 0.116 / 0.123 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a2_b9_n1024 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 7.592 / 8.072 | 0.172 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a2_b9_n1024 | schoolbook |  | 0.929 [0.925, 0.939] | 0.938 [0.925, 0.951] | 7.106 / 7.513 | 0.172 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a32_b32_n16 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 10.564 / 11.111 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a32_b32_n16 | schoolbook |  | 1.288 [1.279, 1.293] | 1.286 [1.267, 1.298] | 13.624 / 14.142 | 0.016 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a32_b32_n1024 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 677.487 / 708.902 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a32_b32_n1024 | schoolbook |  | 1.297 [1.288, 1.303] | 1.295 [1.273, 1.309] | 875.021 / 911.714 | 1.000 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a64_b64_n16 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 37.287 / 39.352 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a64_b64_n16 | schoolbook |  | 1.672 [1.648, 1.685] | 1.657 [1.634, 1.673] | 61.928 / 64.251 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_exact_product | a64_b64_n1024 | crypto_bigint | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2394.156 / 2527.805 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_exact_product | a64_b64_n1024 | schoolbook |  | 1.659 [1.647, 1.681] | 1.662 [1.628, 1.678] | 3976.708 / 4110.399 | 2.000 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l2_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.077 / 0.082 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n16 | fused_exact | yes | 0.800 [0.782, 0.803] | 0.798 [0.773, 0.810] | 0.061 / 0.066 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.823 / 5.069 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l2_n1024 | fused_exact | yes | 0.785 [0.779, 0.795] | 0.793 [0.781, 0.798] | 3.785 / 4.055 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.227 / 0.242 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n16 | fused_exact | yes | 0.963 [0.955, 0.972] | 0.968 [0.950, 0.976] | 0.219 / 0.231 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 14.241 / 15.213 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l4_n1024 | fused_exact | yes | 0.966 [0.954, 0.974] | 0.970 [0.956, 0.983] | 13.772 / 14.761 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n16 | wide_then_add | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.088 / 1.140 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n16 | fused_exact |  | 1.352 [1.343, 1.357] | 1.336 [1.327, 1.353] | 1.466 / 1.540 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_exact_signed_mac | l9_n1024 | wide_then_add | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 69.583 / 73.593 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_exact_signed_mac | l9_n1024 | fused_exact |  | 1.337 [1.323, 1.343] | 1.326 [1.305, 1.353] | 92.610 / 96.539 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_exact_unsigned_mac | l2_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.041 / 0.045 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l2_n16 | fused_exact |  | 0.901 [0.896, 0.917] | 0.899 [0.889, 0.913] | 0.037 / 0.040 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l2_n16 | column_exact | yes | 0.913 [0.815, 0.917] | 0.911 [0.815, 0.920] | 0.037 / 0.039 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l2_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.475 / 2.629 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l2_n1024 | fused_exact |  | 0.874 [0.869, 0.878] | 0.874 [0.869, 0.889] | 2.173 / 2.312 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l2_n1024 | column_exact | yes | 0.850 [0.842, 0.854] | 0.854 [0.844, 0.859] | 2.106 / 2.233 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l4_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.150 / 0.158 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l4_n16 | fused_exact |  | 1.030 [1.019, 1.040] | 1.030 [1.016, 1.036] | 0.154 / 0.162 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_exact_unsigned_mac | l4_n16 | column_exact | yes | 0.868 [0.838, 0.873] | 0.867 [0.851, 0.877] | 0.129 / 0.138 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l4_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 9.287 / 9.878 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l4_n1024 | fused_exact |  | 1.033 [1.026, 1.040] | 1.029 [0.984, 1.040] | 9.569 / 10.197 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_exact_unsigned_mac | l4_n1024 | column_exact | yes | 0.855 [0.836, 0.863] | 0.851 [0.822, 0.863] | 7.925 / 8.401 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l9_n16 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.896 / 0.951 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l9_n16 | fused_exact |  | 1.446 [1.434, 1.453] | 1.430 [1.415, 1.450] | 1.291 / 1.360 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_exact_unsigned_mac | l9_n16 | column_exact | yes | 0.764 [0.753, 0.769] | 0.766 [0.757, 0.773] | 0.683 / 0.729 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l9_n1024 | wide_then_add |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 57.396 / 60.421 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_exact_unsigned_mac | l9_n1024 | fused_exact |  | 1.404 [1.397, 1.414] | 1.405 [1.381, 1.425] | 80.697 / 84.793 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_exact_unsigned_mac | l9_n1024 | column_exact | yes | 0.739 [0.732, 0.758] | 0.745 [0.740, 0.758] | 42.494 / 45.854 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n16 | one_acc | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.060 / 0.064 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n16 | acc4 |  | 1.046 [1.040, 1.058] | 1.043 [1.026, 1.050] | 0.063 / 0.067 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_prime_dot | q100_n16 | length_dispatch |  | 1.002 [0.988, 1.009] | 1.000 [0.987, 1.014] | 0.060 / 0.063 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_dot | q100_n1024 | one_acc | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.440 / 2.599 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n1024 | acc4 |  | 0.983 [0.974, 0.990] | 0.989 [0.977, 0.998] | 2.405 / 2.546 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n1024 | length_dispatch |  | 0.982 [0.974, 0.994] | 0.987 [0.979, 0.995] | 2.406 / 2.550 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n65536 | one_acc | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 155.316 / 163.099 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n65536 | acc4 |  | 0.989 [0.979, 0.996] | 0.989 [0.977, 0.998] | 153.113 / 161.556 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n65536 | length_dispatch |  | 0.987 [0.982, 0.998] | 0.988 [0.965, 0.995] | 153.081 / 162.069 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n16 | one_acc | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.059 / 0.063 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n16 | acc4 |  | 1.052 [1.039, 1.061] | 1.047 [1.039, 1.061] | 0.062 / 0.066 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_prime_dot | q128_n16 | length_dispatch |  | 1.002 [0.996, 1.007] | 1.003 [0.995, 1.011] | 0.060 / 0.063 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_dot | q128_n1024 | one_acc | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.461 / 2.590 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n1024 | acc4 |  | 0.984 [0.977, 0.991] | 0.988 [0.982, 0.992] | 2.422 / 2.546 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n1024 | length_dispatch |  | 0.983 [0.977, 0.987] | 0.982 [0.976, 0.990] | 2.420 / 2.548 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n65536 | one_acc | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 155.715 / 162.992 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n65536 | acc4 |  | 0.989 [0.977, 1.003] | 0.989 [0.975, 0.996] | 153.848 / 162.133 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n65536 | length_dispatch |  | 0.989 [0.976, 0.999] | 0.990 [0.980, 0.999] | 153.374 / 161.751 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n16 | one_acc | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.041 / 0.043 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n16 | acc4 |  | 1.002 [0.992, 1.010] | 1.003 [0.997, 1.016] | 0.041 / 0.043 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_linear | q100_n1024 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.521 / 1.598 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n1024 | acc4 | yes | 0.887 [0.880, 0.901] | 0.892 [0.879, 0.904] | 1.344 / 1.437 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n65536 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 96.667 / 102.756 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n65536 | acc4 | yes | 0.869 [0.845, 0.879] | 0.879 [0.865, 0.891] | 83.798 / 87.590 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n16 | one_acc | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.041 / 0.042 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n16 | acc4 |  | 0.998 [0.994, 1.005] | 0.996 [0.991, 1.004] | 0.041 / 0.043 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n1024 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.539 / 1.615 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n1024 | acc4 | yes | 0.888 [0.881, 0.895] | 0.893 [0.882, 0.900] | 1.365 / 1.438 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n65536 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 97.068 / 102.105 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n65536 | acc4 | yes | 0.870 [0.861, 0.876] | 0.870 [0.860, 0.880] | 83.865 / 88.904 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.170 / 1.251 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | compact_vartime |  | 0.985 [0.969, 0.994] | 0.998 [0.988, 1.004] | 1.156 / 1.229 | 0.000 | 2 / 1536 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | compact_ct |  | 1.279 [1.273, 1.293] | 1.278 [1.264, 1.291] | 1.502 / 1.584 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 27.006 / 28.968 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_vartime |  | 0.970 [0.962, 0.979] | 0.964 [0.944, 0.976] | 26.151 / 27.914 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_ct |  | 0.979 [0.966, 0.991] | 0.980 [0.925, 0.992] | 26.371 / 28.303 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.038 / 1.271 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | compact_vartime |  | 1.118 [1.104, 1.138] | 1.134 [1.102, 1.157] | 1.160 / 1.427 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | compact_ct |  | 1.453 [1.444, 1.473] | 1.450 [1.428, 1.938] | 1.509 / 1.643 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.200 / 22.428 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_vartime |  | 1.228 [1.220, 1.238] | 1.229 [1.207, 1.248] | 25.969 / 27.363 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_ct |  | 1.250 [1.239, 1.257] | 1.253 [1.222, 1.269] | 26.409 / 27.670 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n16 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.085 / 0.091 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q100_zero_n16 | compact_vartime |  | 11.494 [11.345, 11.576] | 11.327 [11.065, 11.550] | 0.979 / 1.023 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n16 | compact_ct |  | 17.653 [17.386, 17.694] | 17.393 [17.032, 18.647] | 1.496 / 1.569 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.268 / 4.538 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_vartime |  | 6.054 [5.984, 6.075] | 5.891 [5.864, 6.013] | 25.585 / 26.985 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_ct |  | 6.175 [6.100, 6.196] | 6.056 [5.981, 6.122] | 26.074 / 27.525 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.304 / 1.397 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | compact_vartime |  | 0.989 [0.984, 1.002] | 0.992 [0.977, 0.999] | 1.301 / 1.378 | 0.000 | 2 / 1536 | inconclusive |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | compact_ct |  | 1.145 [1.137, 1.151] | 1.138 [1.124, 1.154] | 1.494 / 1.586 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 26.724 / 28.382 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_vartime |  | 0.971 [0.965, 0.977] | 0.967 [0.959, 0.975] | 25.930 / 27.917 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_ct |  | 0.975 [0.968, 0.982] | 0.976 [0.969, 0.987] | 25.980 / 28.138 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.173 / 1.255 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | compact_vartime |  | 1.109 [1.102, 1.120] | 1.110 [1.099, 1.126] | 1.300 / 1.380 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | compact_ct |  | 1.281 [1.272, 1.288] | 1.280 [1.256, 1.293] | 1.497 / 1.579 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 20.997 / 22.466 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_vartime |  | 1.239 [1.230, 1.246] | 1.226 [1.215, 1.242] | 25.949 / 27.412 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_ct |  | 1.244 [1.238, 1.249] | 1.235 [1.216, 1.246] | 26.109 / 27.540 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n16 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.085 / 0.090 | 0.000 | 2 / 2560 | pass |
| aarch64 | opt_batch_inverse | q128_zero_n16 | compact_vartime |  | 15.369 [15.234, 15.446] | 15.112 [14.963, 15.301] | 1.301 / 1.359 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n16 | compact_ct |  | 17.628 [17.494, 17.765] | 17.541 [17.239, 17.734] | 1.493 / 1.564 | 0.000 | 2 / 1536 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | production_one_inverse | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.243 / 4.410 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_vartime |  | 6.135 [6.089, 6.152] | 5.998 [5.926, 6.087] | 25.903 / 26.634 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_ct |  | 6.172 [6.131, 6.189] | 6.020 [5.957, 6.129] | 26.055 / 26.746 | 0.000 | 2 / 98304 | regression |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n16 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.085 / 1.155 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n16 | compact_ct |  | 1.327 [1.319, 1.335] | 1.318 [1.308, 1.327] | 1.437 / 1.521 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n1024 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.718 / 26.409 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n1024 | compact_ct |  | 1.012 [1.003, 1.021] | 1.013 [1.006, 1.023] | 24.891 / 26.825 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n16 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.092 / 1.154 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n16 | compact_ct |  | 1.330 [1.319, 1.337] | 1.329 [1.312, 1.347] | 1.448 / 1.529 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n1024 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.510 / 26.482 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n1024 | compact_ct |  | 1.013 [1.007, 1.021] | 1.014 [0.998, 1.038] | 24.978 / 26.957 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n16 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.909 / 0.969 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n16 | compact_ct |  | 1.588 [1.580, 1.604] | 1.566 [1.544, 1.580] | 1.438 / 1.523 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n1024 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.254 / 26.251 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n1024 | compact_ct |  | 1.029 [1.008, 1.033] | 1.024 [1.012, 1.037] | 24.876 / 25.992 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n16 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.224 / 1.308 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n16 | compact_ct |  | 1.171 [1.162, 1.181] | 1.166 [1.149, 1.173] | 1.433 / 1.527 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n1024 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.424 / 26.342 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n1024 | compact_ct |  | 1.018 [1.005, 1.036] | 1.011 [0.987, 1.023] | 24.854 / 26.635 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n16 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.236 / 1.311 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n16 | compact_ct |  | 1.170 [1.162, 1.181] | 1.182 [1.156, 1.196] | 1.444 / 1.525 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n1024 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.254 / 26.237 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n1024 | compact_ct |  | 1.013 [1.005, 1.020] | 1.014 [1.008, 1.026] | 24.736 / 26.368 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n16 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.225 / 1.265 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n16 | compact_ct |  | 1.170 [1.164, 1.175] | 1.164 [1.152, 1.185] | 1.431 / 1.477 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n1024 | compact_vartime | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.335 / 25.513 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n1024 | compact_ct |  | 1.018 [1.007, 1.026] | 1.014 [1.004, 1.021] | 24.873 / 25.751 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_public_pow | e17_n16 | bounded_window |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6.747 / 6.999 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e17_n16 | public_binary | yes | 0.373 [0.371, 0.375] | 0.379 [0.373, 0.397] | 2.515 / 2.705 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e17_n1024 | bounded_window |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 428.633 / 443.468 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e17_n1024 | public_binary | yes | 0.373 [0.373, 0.375] | 0.383 [0.374, 0.394] | 160.099 / 171.305 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e127_n16 | bounded_window |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 31.054 / 32.169 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e127_n16 | public_binary | yes | 0.712 [0.708, 0.715] | 0.717 [0.700, 0.727] | 22.102 / 23.072 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e127_n1024 | bounded_window |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1993.761 / 2063.996 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_prime_public_pow | e127_n1024 | public_binary | yes | 0.712 [0.706, 0.718] | 0.717 [0.699, 0.722] | 1417.844 / 1473.228 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n16 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n16 | prepared_formula |  | 1.010 [1.004, 1.020] | 1.006 [0.991, 1.026] | 0.013 / 0.013 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | zero_n16 | public_scalar_dispatch | yes | 0.599 [0.596, 0.604] | 0.611 [0.595, 0.622] | 0.008 / 0.008 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n1024 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.743 / 0.759 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n1024 | prepared_formula |  | 0.998 [0.994, 1.004] | 1.010 [0.993, 1.022] | 0.742 / 0.765 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | zero_n1024 | public_scalar_dispatch | yes | 0.459 [0.455, 0.460] | 0.469 [0.457, 0.481] | 0.340 / 0.355 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n65536 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 47.634 / 49.360 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | zero_n65536 | prepared_formula |  | 1.007 [0.999, 1.014] | 1.019 [1.001, 1.033] | 47.984 / 50.418 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | zero_n65536 | public_scalar_dispatch | yes | 0.445 [0.440, 0.450] | 0.453 [0.446, 0.460] | 21.165 / 22.337 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n16 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n16 | prepared_formula |  | 1.011 [1.005, 1.017] | 1.012 [1.000, 1.022] | 0.013 / 0.013 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | half_n16 | public_scalar_dispatch | yes | 0.730 [0.727, 0.739] | 0.739 [0.729, 0.757] | 0.009 / 0.010 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n1024 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.743 / 0.765 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n1024 | prepared_formula |  | 1.003 [0.999, 1.009] | 1.002 [0.987, 1.021] | 0.748 / 0.767 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | half_n1024 | public_scalar_dispatch | yes | 0.613 [0.611, 0.616] | 0.629 [0.614, 0.639] | 0.456 / 0.480 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n65536 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 47.674 / 51.498 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | half_n65536 | prepared_formula |  | 1.003 [0.999, 1.017] | 1.007 [0.981, 1.022] | 47.935 / 52.407 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | half_n65536 | public_scalar_dispatch | yes | 0.609 [0.607, 0.615] | 0.619 [0.594, 0.625] | 29.061 / 32.001 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | full_n16 | actual_fixed_gf | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | full_n16 | prepared_formula |  | 1.010 [1.005, 1.016] | 1.018 [1.002, 1.066] | 0.013 / 0.013 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | full_n16 | public_scalar_dispatch |  | 1.177 [1.172, 1.186] | 1.177 [1.167, 1.204] | 0.015 / 0.016 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_gf_fixed | full_n1024 | actual_fixed_gf | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.744 / 0.767 | 0.031 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | full_n1024 | prepared_formula |  | 1.006 [0.999, 1.012] | 1.010 [1.003, 1.022] | 0.748 / 0.774 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | full_n1024 | public_scalar_dispatch |  | 1.113 [1.106, 1.120] | 1.113 [1.087, 1.125] | 0.828 / 0.844 | 0.031 | 0 / 0 | regression |
| aarch64 | opt_gf_fixed | full_n65536 | actual_fixed_gf | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 48.032 / 51.027 | 2.000 | 0 / 0 | pass |
| aarch64 | opt_gf_fixed | full_n65536 | prepared_formula |  | 1.005 [0.998, 1.016] | 1.004 [0.904, 1.016] | 48.379 / 51.219 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_fixed | full_n65536 | public_scalar_dispatch |  | 1.112 [1.104, 1.126] | 1.110 [1.094, 1.323] | 53.539 / 74.857 | 2.000 | 0 / 0 | regression |
| aarch64 | opt_gf_butterfly | zero_n16 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.016 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | zero_n16 | public_scalar_dispatch | yes | 0.841 [0.824, 0.844] | 0.848 [0.839, 0.853] | 0.013 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | zero_n1024 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.917 / 0.942 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | zero_n1024 | public_scalar_dispatch | yes | 0.833 [0.827, 0.841] | 0.833 [0.819, 0.846] | 0.764 / 0.784 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | zero_n65536 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 59.472 / 61.410 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | zero_n65536 | public_scalar_dispatch | yes | 0.828 [0.817, 0.837] | 0.825 [0.818, 0.849] | 49.139 / 51.041 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n16 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.016 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n16 | public_scalar_dispatch | yes | 0.848 [0.841, 0.857] | 0.852 [0.843, 0.858] | 0.013 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n1024 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.919 / 0.941 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n1024 | public_scalar_dispatch | yes | 0.742 [0.739, 0.747] | 0.746 [0.740, 0.767] | 0.683 / 0.712 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n65536 | actual_fixed_gf |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 59.350 / 64.195 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | half_n65536 | public_scalar_dispatch | yes | 0.745 [0.737, 0.765] | 0.750 [0.739, 0.813] | 44.217 / 51.919 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | full_n16 | actual_fixed_gf | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.016 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | full_n16 | public_scalar_dispatch |  | 1.223 [1.214, 1.226] | 1.215 [1.197, 1.228] | 0.019 / 0.020 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_gf_butterfly | full_n1024 | actual_fixed_gf | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.922 / 0.950 | 0.062 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | full_n1024 | public_scalar_dispatch |  | 1.162 [1.157, 1.170] | 1.168 [1.125, 1.181] | 1.072 / 1.109 | 0.062 | 0 / 0 | regression |
| aarch64 | opt_gf_butterfly | full_n65536 | actual_fixed_gf | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 59.861 / 65.991 | 4.000 | 0 / 0 | pass |
| aarch64 | opt_gf_butterfly | full_n65536 | public_scalar_dispatch |  | 1.157 [1.153, 1.178] | 1.161 [1.144, 1.596] | 69.510 / 161.776 | 4.000 | 0 / 0 | regression |
| aarch64 | opt_gf_round | 16 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.093 / 0.096 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 16 | wide1 | yes | 0.812 [0.808, 0.819] | 0.812 [0.801, 0.824] | 0.076 / 0.078 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 16 | wide2 |  | 0.875 [0.869, 0.879] | 0.872 [0.857, 0.886] | 0.081 / 0.084 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 1024 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 5.663 / 5.781 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 1024 | wide1 | yes | 0.815 [0.811, 0.817] | 0.821 [0.786, 0.827] | 4.616 / 4.710 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 1024 | wide2 |  | 0.847 [0.841, 0.850] | 0.851 [0.818, 0.865] | 4.792 / 4.918 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 65536 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 362.427 / 379.425 | 5.000 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 65536 | wide1 | yes | 0.816 [0.812, 0.825] | 0.813 [0.803, 0.826] | 296.786 / 307.154 | 5.000 | 0 / 0 | pass |
| aarch64 | opt_gf_round | 65536 | wide2 |  | 0.859 [0.852, 0.864] | 0.864 [0.852, 0.885] | 311.098 / 329.061 | 5.000 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 16 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.024 / 0.025 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 16 | native_batch | yes | 0.134 [0.133, 0.135] | 0.140 [0.135, 0.158] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 1024 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.503 / 1.812 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 1024 | native_batch | yes | 0.046 [0.044, 0.046] | 0.047 [0.044, 0.047] | 0.069 / 0.074 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 65536 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 96.396 / 100.952 | 0.188 | 0 / 0 | pass |
| aarch64 | opt_gf8_mul | 65536 | native_batch | yes | 0.046 [0.046, 0.047] | 0.049 [0.046, 0.051] | 4.447 / 5.034 | 0.188 | 0 / 0 | pass |
| aarch64 | opt_phi8 | 16 | table_public_input | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.009 / 0.010 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_phi8 | 16 | fixed_basis |  | 10.729 [9.609, 11.247] | 10.585 [9.578, 10.935] | 0.095 / 0.097 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_phi8 | 1024 | table_public_input | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.376 / 0.423 | 0.017 | 0 / 0 | pass |
| aarch64 | opt_phi8 | 1024 | fixed_basis |  | 15.947 [15.772, 16.038] | 16.155 [15.227, 19.737] | 5.946 / 8.259 | 0.017 | 0 / 0 | regression |
| aarch64 | opt_phi8 | 65536 | table_public_input | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 23.766 / 26.927 | 1.062 | 0 / 0 | pass |
| aarch64 | opt_phi8 | 65536 | fixed_basis |  | 16.062 [15.919, 16.233] | 15.777 [14.912, 16.663] | 381.800 / 404.545 | 1.062 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 16 | production | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.020 / 0.020 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_b127_mul | 16 | karatsuba |  | 1.299 [1.295, 1.304] | 1.296 [1.284, 1.731] | 0.025 / 0.026 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 16 | pfold |  | 1.294 [1.287, 1.297] | 1.287 [1.273, 1.303] | 0.025 / 0.026 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 1024 | production | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.241 / 1.324 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_b127_mul | 1024 | karatsuba |  | 1.292 [1.282, 1.300] | 1.283 [1.265, 1.342] | 1.599 / 1.668 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 1024 | pfold |  | 1.304 [1.291, 1.313] | 1.303 [1.280, 1.390] | 1.612 / 1.799 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 65536 | production | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 80.039 / 84.832 | 3.000 | 0 / 0 | pass |
| aarch64 | opt_b127_mul | 65536 | karatsuba |  | 1.298 [1.286, 1.312] | 1.278 [1.266, 1.309] | 103.813 / 109.053 | 3.000 | 0 / 0 | regression |
| aarch64 | opt_b127_mul | 65536 | pfold |  | 1.308 [1.294, 1.326] | 1.300 [1.272, 1.331] | 105.080 / 109.542 | 3.000 | 0 / 0 | regression |
| aarch64 | opt_ood | log10 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.098 / 3.225 | 0.016 | 13 / 32832 | pass |
| aarch64 | opt_ood | log10 | reuse_products | yes | 0.718 [0.711, 0.720] | 0.723 [0.708, 0.735] | 2.218 / 2.323 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | indexed_products |  | 0.747 [0.740, 0.751] | 0.747 [0.742, 0.756] | 2.310 / 2.416 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood | log10 | collect_products |  | 0.757 [0.751, 0.763] | 0.756 [0.751, 0.790] | 2.345 / 2.465 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | vector_products |  | 0.753 [0.748, 0.763] | 0.765 [0.750, 0.773] | 2.340 / 2.442 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log10 | prepared_products |  | 0.744 [0.738, 0.750] | 0.753 [0.740, 0.757] | 2.307 / 2.412 | 0.016 | 3 / 16464 | pass |
| aarch64 | opt_ood | log16 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 68.174 / 86.237 | 1.000 | 20 / 133328 | pass |
| aarch64 | opt_ood | log16 | reuse_products | yes | 0.893 [0.881, 0.919] | 0.883 [0.819, 0.958] | 60.887 / 79.194 | 1.000 | 3 / 67312 | pass |
| aarch64 | opt_ood | log16 | indexed_products |  | 0.965 [0.947, 0.976] | 0.963 [0.859, 1.033] | 65.767 / 84.725 | 1.000 | 3 / 67312 | inconclusive |
| aarch64 | opt_ood | log16 | collect_products |  | 0.966 [0.956, 1.002] | 0.980 [0.833, 1.021] | 65.860 / 85.155 | 1.000 | 4 / 67568 | inconclusive |
| aarch64 | opt_ood | log16 | vector_products |  | 0.977 [0.966, 1.004] | 0.980 [0.929, 1.037] | 66.718 / 86.051 | 1.000 | 4 / 67568 | inconclusive |
| aarch64 | opt_ood | log16 | prepared_products |  | 0.977 [0.961, 0.991] | 0.952 [0.801, 1.014] | 66.222 / 83.962 | 1.000 | 4 / 67568 | inconclusive |
| aarch64 | opt_ood | log18 | production_blocked |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 224.131 / 246.908 | 4.000 | 22 / 135632 | pass |
| aarch64 | opt_ood | log18 | reuse_products | yes | 0.895 [0.886, 0.904] | 0.904 [0.883, 0.920] | 200.958 / 222.647 | 4.000 | 3 / 68080 | pass |
| aarch64 | opt_ood | log18 | indexed_products |  | 0.988 [0.979, 0.998] | 0.979 [0.954, 1.020] | 221.008 / 245.065 | 4.000 | 3 / 68080 | inconclusive |
| aarch64 | opt_ood | log18 | collect_products |  | 0.992 [0.983, 1.003] | 0.985 [0.961, 1.021] | 223.128 / 243.799 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | vector_products |  | 1.002 [0.995, 1.011] | 1.008 [0.972, 1.060] | 224.958 / 249.823 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood | log18 | prepared_products |  | 0.984 [0.979, 0.994] | 0.983 [0.959, 1.013] | 221.801 / 244.178 | 4.000 | 4 / 69104 | inconclusive |
| aarch64 | opt_ood_reuse | log10 | allocate |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.209 / 2.272 | 0.016 | 2 / 16400 | pass |
| aarch64 | opt_ood_reuse | log10 | scratch | yes | 0.831 [0.820, 0.835] | 0.836 [0.826, 0.844] | 1.839 / 1.883 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_ood_reuse | log10 | indexed_scratch |  | 0.860 [0.853, 0.864] | 0.864 [0.853, 0.869] | 1.903 / 1.951 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_ood_reuse | log16 | allocate | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 60.749 / 69.546 | 1.000 | 3 / 67312 | pass |
| aarch64 | opt_ood_reuse | log16 | scratch |  | 0.965 [0.932, 0.982] | 0.990 [0.926, 1.020] | 58.768 / 67.063 | 1.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ood_reuse | log16 | indexed_scratch |  | 1.054 [1.021, 1.069] | 1.055 [1.016, 1.095] | 64.158 / 72.125 | 1.000 | 1 / 1520 | regression |
| aarch64 | opt_ood_reuse | log18 | allocate | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 200.241 / 223.831 | 4.000 | 3 / 68080 | pass |
| aarch64 | opt_ood_reuse | log18 | scratch |  | 0.990 [0.978, 1.006] | 1.013 [0.971, 1.140] | 199.460 / 220.199 | 4.000 | 1 / 1520 | inconclusive |
| aarch64 | opt_ood_reuse | log18 | indexed_scratch |  | 1.099 [1.093, 1.125] | 1.117 [1.093, 1.572] | 222.669 / 251.378 | 4.000 | 1 / 1520 | regression |
| aarch64 | opt_ntt | log8_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 46.258 / 47.894 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | depth_first |  | 0.866 [0.862, 0.869] | 0.861 [0.854, 0.873] | 40.084 / 41.097 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | tiled_half |  | 0.662 [0.659, 0.666] | 0.667 [0.657, 0.679] | 30.681 / 32.046 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes32 | half_depth | yes | 0.663 [0.659, 0.667] | 0.659 [0.654, 0.675] | 30.686 / 31.617 | 0.375 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 405.076 / 429.804 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | depth_first |  | 0.904 [0.891, 0.922] | 0.898 [0.882, 0.919] | 366.443 / 384.837 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | tiled_half |  | 0.812 [0.803, 0.828] | 0.802 [0.791, 0.835] | 327.800 / 355.142 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log12_lanes8 | half_depth | yes | 0.811 [0.799, 0.828] | 0.806 [0.682, 0.829] | 328.036 / 350.336 | 1.500 | 0 / 0 | pass |
| aarch64 | opt_ntt | log15_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3798.312 / 3981.206 | 12.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes8 | depth_first |  | 0.925 [0.913, 0.937] | 0.929 [0.899, 0.948] | 3520.146 / 3638.333 | 12.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log15_lanes8 | tiled_half |  | 0.859 [0.848, 0.867] | 0.856 [0.838, 0.865] | 3252.479 / 3380.021 | 12.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log15_lanes8 | half_depth | yes | 0.853 [0.846, 0.862] | 0.855 [0.836, 0.870] | 3245.250 / 3381.648 | 12.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log15_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 11180.792 / 12236.192 | 48.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes32 | depth_first |  | 0.926 [0.916, 0.937] | 0.977 [0.838, 1.086] | 10369.312 / 11470.273 | 48.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log15_lanes32 | tiled_half |  | 0.856 [0.850, 0.870] | 0.866 [0.834, 0.954] | 9554.624 / 11127.525 | 48.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log15_lanes32 | half_depth | yes | 0.854 [0.849, 0.862] | 0.856 [0.799, 0.935] | 9522.104 / 10819.102 | 48.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log16_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 23745.000 / 26295.990 | 96.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log16_lanes32 | depth_first |  | 0.942 [0.926, 0.956] | 0.973 [0.939, 1.033] | 22485.062 / 25232.932 | 96.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | tiled_half |  | 0.877 [0.867, 0.888] | 0.923 [0.886, 0.971] | 20931.666 / 23358.754 | 96.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log16_lanes32 | half_depth | yes | 0.877 [0.867, 0.886] | 0.895 [0.876, 0.960] | 20937.979 / 23745.746 | 96.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log18_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 106120.083 / 116926.738 | 384.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log18_lanes32 | depth_first |  | 0.944 [0.936, 0.952] | 0.947 [0.850, 0.977] | 100078.917 / 111032.469 | 384.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log18_lanes32 | tiled_half |  | 0.887 [0.879, 0.903] | 0.890 [0.822, 0.966] | 94356.083 / 104191.656 | 384.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log18_lanes32 | half_depth | yes | 0.885 [0.878, 0.900] | 0.890 [0.797, 0.915] | 94161.938 / 103643.760 | 384.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes1 | production | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.662 / 3.053 | 0.012 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | 1.359 [1.326, 1.385] | 1.417 [1.301, 1.529] | 3.606 / 4.314 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | tiled_half |  | 1.140 [1.093, 1.159] | 1.158 [1.026, 1.186] | 3.020 / 3.387 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_ntt | log8_lanes1 | half_depth |  | 1.137 [1.101, 1.151] | 1.101 [1.045, 1.138] | 3.008 / 3.304 | 0.012 | 0 / 0 | regression |
| aarch64 | opt_pack | logs9_9 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24.981 / 31.429 | 0.016 | 2 / 17904 | pass |
| aarch64 | opt_pack | logs9_9 | single_write |  | 0.034 [0.026, 0.130] | 0.027 [0.018, 0.031] | 0.731 / 0.797 | 0.016 | 1 / 16384 | pass |
| aarch64 | opt_pack | logs9_9 | tiled_write | yes | 0.027 [0.020, 0.107] | 0.022 [0.019, 0.029] | 0.572 / 0.671 | 0.016 | 1 / 16384 | pass |
| aarch64 | opt_pack | logs16_14 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 127.649 / 152.782 | 2.000 | 2 / 2098672 | pass |
| aarch64 | opt_pack | logs16_14 | single_write |  | 0.739 [0.722, 0.841] | 0.746 [0.659, 0.794] | 95.917 / 103.365 | 2.000 | 1 / 2097152 | pass |
| aarch64 | opt_pack | logs16_14 | tiled_write | yes | 0.531 [0.520, 0.606] | 0.518 [0.423, 0.565] | 68.979 / 73.018 | 2.000 | 1 / 2097152 | pass |
| aarch64 | opt_pack | logs18_18 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 407.172 / 449.624 | 8.000 | 2 / 8390128 | pass |
| aarch64 | opt_pack | logs18_18 | single_write |  | 1.078 [1.070, 1.157] | 1.043 [0.999, 1.119] | 444.646 / 463.588 | 8.000 | 1 / 8388608 | regression |
| aarch64 | opt_pack | logs18_18 | tiled_write | yes | 0.685 [0.674, 0.714] | 0.701 [0.663, 0.738] | 279.680 / 308.321 | 8.000 | 1 / 8388608 | pass |
| aarch64 | opt_packed_ood | logs9_9 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 34.391 / 53.876 | 0.031 | 15 / 50736 | pass |
| aarch64 | opt_packed_ood | logs9_9 | single_write_ood |  | 0.090 [0.076, 0.345] | 0.076 [0.070, 0.082] | 2.963 / 4.168 | 0.031 | 3 / 32784 | pass |
| aarch64 | opt_packed_ood | logs9_9 | tiled_indexed_ood | yes | 0.089 [0.076, 0.337] | 0.076 [0.067, 0.083] | 2.914 / 3.581 | 0.031 | 3 / 32784 | pass |
| aarch64 | opt_packed_ood | logs16_14 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 263.328 / 280.660 | 4.000 | 22 / 2231248 | pass |
| aarch64 | opt_packed_ood | logs16_14 | single_write_ood |  | 1.099 [0.895, 1.116] | 1.056 [0.939, 1.115] | 287.212 / 305.395 | 4.000 | 4 / 2164720 | inconclusive |
| aarch64 | opt_packed_ood | logs16_14 | tiled_indexed_ood | yes | 0.793 [0.779, 0.835] | 0.777 [0.757, 0.840] | 206.279 / 220.480 | 4.000 | 4 / 2164720 | pass |
| aarch64 | opt_packed_ood | logs18_18 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 848.640 / 927.369 | 16.000 | 24 / 8527312 | pass |
| aarch64 | opt_packed_ood | logs18_18 | single_write_ood |  | 1.080 [1.034, 1.192] | 1.170 [1.044, 1.189] | 947.453 / 1037.941 | 16.000 | 4 / 8457712 | regression |
| aarch64 | opt_packed_ood | logs18_18 | tiled_indexed_ood | yes | 0.872 [0.845, 0.880] | 0.866 [0.823, 0.894] | 738.391 / 787.424 | 16.000 | 4 / 8457712 | pass |
| aarch64 | opt_gf_two_pair | 16 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.184 / 0.197 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_gf_two_pair | 16 | separate_wide | yes | 0.832 [0.824, 0.836] | 0.833 [0.821, 0.879] | 0.153 / 0.164 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_gf_two_pair | 1024 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 11.219 / 17.896 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_gf_two_pair | 1024 | separate_wide | yes | 0.831 [0.826, 0.866] | 0.830 [0.684, 0.836] | 9.306 / 15.614 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_gf_fold_round | 16 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.185 / 0.197 | 0.004 | 0 / 0 | pass |
| aarch64 | opt_gf_fold_round | 16 | fold_then_wide | yes | 0.923 [0.920, 0.932] | 0.929 [0.915, 0.940] | 0.171 / 0.180 | 0.004 | 0 / 0 | pass |
| aarch64 | opt_gf_fold_round | 1024 | production_fused |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 12.081 / 14.713 | 0.250 | 0 / 0 | pass |
| aarch64 | opt_gf_fold_round | 1024 | fold_then_wide | yes | 0.940 [0.928, 0.947] | 0.947 [0.929, 0.971] | 11.341 / 14.372 | 0.250 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | production_fused | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.828 / 0.852 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 16 | chunked |  | 1.012 [1.006, 1.022] | 1.015 [1.006, 1.022] | 0.838 / 0.861 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_gf_grid | 1024 | production_fused | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 53.807 / 56.816 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_gf_grid | 1024 | chunked |  | 1.039 [1.027, 1.050] | 1.039 [1.030, 1.068] | 55.748 / 59.838 | 1.000 | 0 / 0 | regression |
| aarch64 | opt_gf8_inverse | 16 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.584 / 0.609 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf8_inverse | 16 | vector_chain | yes | 0.051 [0.050, 0.051] | 0.052 [0.051, 0.056] | 0.030 / 0.032 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_gf8_inverse | 1024 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 37.338 / 40.661 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_gf8_inverse | 1024 | vector_chain | yes | 0.044 [0.043, 0.044] | 0.045 [0.043, 0.046] | 1.630 / 1.794 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_gf8_inverse | 65536 | flock_scalar |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2410.573 / 2572.875 | 0.125 | 0 / 0 | pass |
| aarch64 | opt_gf8_inverse | 65536 | vector_chain | yes | 0.044 [0.043, 0.045] | 0.045 [0.044, 0.047] | 106.188 / 118.780 | 0.125 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | fixed_schedule |  | 0.482 [0.475, 0.485] | 0.483 [0.465, 0.504] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | public_fused |  | 0.975 [0.962, 0.983] | 0.959 [0.943, 0.978] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | public_adaptive |  | 0.648 [0.639, 0.653] | 0.655 [0.621, 0.662] | 0.008 / 0.008 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | row_density |  | 0.962 [0.951, 0.970] | 0.961 [0.937, 0.973] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.660 / 0.787 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | fixed_schedule |  | 0.385 [0.380, 0.387] | 0.391 [0.361, 0.399] | 0.254 / 0.286 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | public_fused |  | 0.974 [0.967, 0.979] | 0.974 [0.843, 1.004] | 0.642 / 0.746 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | public_adaptive |  | 0.389 [0.385, 0.393] | 0.393 [0.356, 0.403] | 0.256 / 0.327 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | row_density | yes | 0.975 [0.968, 0.982] | 0.969 [0.947, 1.002] | 0.643 / 0.768 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.016 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | fixed_schedule |  | 0.459 [0.395, 0.467] | 0.459 [0.408, 0.490] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | public_fused |  | 0.750 [0.665, 0.758] | 0.748 [0.673, 0.762] | 0.010 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | public_adaptive |  | 0.855 [0.749, 0.859] | 0.854 [0.766, 0.863] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | row_density |  | 0.751 [0.668, 0.754] | 0.744 [0.678, 0.768] | 0.010 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.741 / 0.843 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | fixed_schedule |  | 0.348 [0.306, 0.364] | 0.343 [0.304, 0.366] | 0.254 / 0.272 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | public_fused |  | 0.643 [0.563, 0.689] | 0.647 [0.562, 0.685] | 0.474 / 0.517 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | public_adaptive |  | 0.643 [0.565, 0.686] | 0.658 [0.567, 0.691] | 0.476 / 0.509 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | row_density | yes | 0.641 [0.553, 0.681] | 0.634 [0.568, 0.685] | 0.474 / 0.499 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.142 / 0.153 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | fixed_schedule |  | 0.606 [0.599, 0.610] | 0.607 [0.425, 0.614] | 0.086 / 0.093 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_fused |  | 1.109 [1.099, 1.115] | 1.098 [0.763, 1.110] | 0.157 / 0.170 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_adaptive |  | 0.625 [0.617, 0.629] | 0.628 [0.453, 0.638] | 0.089 / 0.098 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | row_density |  | 0.633 [0.627, 0.644] | 0.636 [0.434, 0.641] | 0.090 / 0.097 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.337 / 8.884 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | fixed_schedule |  | 0.796 [0.787, 0.805] | 0.797 [0.785, 0.818] | 6.646 / 7.120 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_fused |  | 1.174 [1.166, 1.186] | 1.169 [1.141, 1.192] | 9.808 / 10.433 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_adaptive |  | 0.798 [0.794, 0.807] | 0.799 [0.768, 0.815] | 6.657 / 7.073 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | row_density |  | 0.672 [0.662, 0.676] | 0.664 [0.656, 0.688] | 5.600 / 6.047 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.058 / 0.071 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | fixed_schedule |  | 1.474 [1.201, 1.712] | 1.482 [1.193, 1.671] | 0.086 / 0.090 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_fused |  | 0.883 [0.849, 0.899] | 0.883 [0.855, 0.906] | 0.050 / 0.063 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_adaptive |  | 0.890 [0.858, 0.903] | 0.896 [0.868, 0.911] | 0.051 / 0.064 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | row_density |  | 1.017 [0.938, 1.108] | 1.018 [0.937, 1.114] | 0.058 / 0.074 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.622 / 3.914 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | fixed_schedule |  | 1.874 [1.792, 1.883] | 1.800 [1.700, 1.946] | 6.640 / 6.930 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_fused |  | 0.840 [0.834, 0.855] | 0.828 [0.794, 0.853] | 3.061 / 3.249 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_adaptive |  | 0.854 [0.844, 0.860] | 0.835 [0.798, 0.863] | 3.074 / 3.249 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | row_density |  | 1.459 [1.437, 1.495] | 1.411 [1.363, 1.452] | 5.317 / 5.538 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.971 / 2.087 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | fixed_schedule |  | 0.425 [0.422, 0.429] | 0.433 [0.419, 0.437] | 0.838 / 0.901 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | public_fused |  | 0.986 [0.969, 1.000] | 0.987 [0.967, 1.006] | 1.940 / 2.057 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | public_adaptive |  | 0.429 [0.422, 0.431] | 0.432 [0.425, 0.441] | 0.843 / 0.920 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | row_density | yes | 0.427 [0.419, 0.430] | 0.427 [0.420, 0.437] | 0.838 / 0.894 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 126.206 / 134.557 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | fixed_schedule |  | 0.426 [0.419, 0.430] | 0.424 [0.409, 0.437] | 53.540 / 57.864 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | public_fused |  | 0.993 [0.987, 1.011] | 0.977 [0.944, 1.022] | 125.937 / 132.664 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | public_adaptive |  | 0.422 [0.419, 0.429] | 0.426 [0.411, 0.439] | 53.549 / 58.011 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | row_density | yes | 0.425 [0.421, 0.430] | 0.424 [0.408, 0.432] | 53.488 / 58.405 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.224 / 0.247 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n16 | fixed_schedule |  | 3.733 [3.460, 4.413] | 4.099 [3.465, 4.369] | 0.843 / 0.867 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n16 | public_fused |  | 0.962 [0.956, 0.977] | 0.973 [0.953, 0.998] | 0.216 / 0.239 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n16 | public_adaptive |  | 0.912 [0.843, 0.932] | 0.908 [0.849, 0.927] | 0.204 / 0.214 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n16 | row_density | yes | 0.976 [0.950, 0.993] | 0.992 [0.960, 1.006] | 0.218 / 0.236 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 20.235 / 33.033 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n1024 | fixed_schedule |  | 2.677 [2.487, 2.813] | 1.734 [1.582, 1.850] | 54.013 / 56.470 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n1024 | public_fused |  | 1.189 [1.125, 1.305] | 1.101 [1.024, 1.167] | 25.088 / 36.309 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n1024 | public_adaptive |  | 0.715 [0.669, 0.803] | 0.774 [0.674, 0.821] | 14.814 / 25.908 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n1024 | row_density | yes | 0.871 [0.775, 0.958] | 0.751 [0.695, 0.820] | 16.947 / 25.719 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n16 | fixed_schedule |  | 0.456 [0.454, 0.461] | 0.462 [0.455, 0.867] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n16 | public_fused |  | 0.651 [0.646, 0.658] | 0.665 [0.644, 0.803] | 0.008 / 0.009 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n16 | public_adaptive |  | 0.754 [0.745, 0.759] | 0.758 [0.744, 0.796] | 0.010 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n16 | row_density |  | 0.650 [0.644, 0.654] | 0.660 [0.648, 0.717] | 0.008 / 0.009 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.469 / 0.501 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n1024 | fixed_schedule |  | 0.542 [0.528, 0.547] | 0.547 [0.535, 0.558] | 0.254 / 0.271 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n1024 | public_fused |  | 0.722 [0.706, 0.726] | 0.720 [0.579, 0.732] | 0.337 / 0.363 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n1024 | public_adaptive |  | 0.724 [0.706, 0.730] | 0.720 [0.573, 0.739] | 0.337 / 0.364 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n1024 | row_density | yes | 0.714 [0.707, 0.725] | 0.719 [0.703, 0.749] | 0.336 / 0.365 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n16 | fixed_schedule |  | 0.531 [0.507, 0.538] | 0.527 [0.507, 0.550] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n16 | public_fused |  | 0.824 [0.798, 0.884] | 0.810 [0.797, 0.884] | 0.010 / 0.010 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n16 | public_adaptive |  | 0.722 [0.680, 0.725] | 0.686 [0.679, 0.729] | 0.008 / 0.009 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n16 | row_density |  | 0.822 [0.753, 0.881] | 0.828 [0.752, 0.951] | 0.010 / 0.010 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.779 / 0.876 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n1024 | fixed_schedule |  | 0.329 [0.307, 0.353] | 0.319 [0.305, 0.354] | 0.253 / 0.273 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n1024 | public_fused |  | 0.629 [0.580, 0.668] | 0.648 [0.585, 0.698] | 0.481 / 0.515 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n1024 | public_adaptive |  | 0.333 [0.314, 0.359] | 0.336 [0.316, 0.370] | 0.255 / 0.279 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n1024 | row_density | yes | 0.629 [0.574, 0.663] | 0.628 [0.584, 0.677] | 0.479 / 0.520 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n16 | fixed_schedule |  | 0.497 [0.494, 0.503] | 0.500 [0.330, 0.511] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n16 | public_fused |  | 0.900 [0.892, 0.909] | 0.892 [0.636, 1.001] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n16 | public_adaptive |  | 0.947 [0.941, 0.954] | 0.938 [0.723, 1.185] | 0.011 / 0.014 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n16 | row_density |  | 0.896 [0.891, 0.907] | 0.894 [0.637, 1.369] | 0.011 / 0.013 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.667 / 0.800 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n1024 | fixed_schedule |  | 0.388 [0.383, 0.392] | 0.397 [0.248, 0.411] | 0.258 / 0.320 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n1024 | public_fused |  | 0.976 [0.963, 0.984] | 0.971 [0.922, 1.124] | 0.647 / 0.843 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n1024 | public_adaptive |  | 0.979 [0.960, 0.986] | 0.979 [0.581, 1.026] | 0.650 / 0.769 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n1024 | row_density | yes | 0.979 [0.967, 0.988] | 0.970 [0.543, 0.989] | 0.651 / 0.845 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n16 | fixed_schedule |  | 0.473 [0.468, 0.480] | 0.480 [0.432, 0.490] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n16 | public_fused |  | 1.091 [1.079, 1.106] | 1.091 [1.067, 1.131] | 0.014 / 0.015 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n16 | public_adaptive |  | 0.999 [0.986, 1.010] | 0.989 [0.974, 1.025] | 0.012 / 0.013 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n16 | row_density |  | 1.103 [1.082, 1.113] | 1.079 [1.060, 1.187] | 0.014 / 0.015 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.668 / 0.720 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n1024 | fixed_schedule |  | 0.380 [0.378, 0.383] | 0.381 [0.361, 0.393] | 0.254 / 0.275 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n1024 | public_fused |  | 0.487 [0.485, 0.491] | 0.486 [0.436, 0.499] | 0.325 / 0.349 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n1024 | public_adaptive |  | 0.491 [0.487, 0.496] | 0.498 [0.478, 0.510] | 0.327 / 0.356 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n1024 | row_density | yes | 0.488 [0.484, 0.495] | 0.492 [0.459, 0.515] | 0.327 / 0.353 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.020 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | fixed_schedule |  | 4.247 [4.225, 4.360] | 4.202 [4.129, 4.338] | 0.087 / 0.090 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_fused |  | 0.931 [0.923, 0.936] | 0.932 [0.917, 0.947] | 0.019 / 0.020 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_adaptive |  | 0.963 [0.956, 0.968] | 0.965 [0.952, 0.981] | 0.020 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | row_density |  | 0.432 [0.426, 0.436] | 0.447 [0.429, 0.452] | 0.009 / 0.010 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.117 / 1.174 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | fixed_schedule |  | 5.944 [5.922, 5.979] | 5.885 [5.808, 5.973] | 6.626 / 6.910 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_fused |  | 0.884 [0.879, 0.893] | 0.894 [0.881, 0.909] | 0.991 / 1.053 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_adaptive |  | 0.895 [0.885, 0.902] | 0.896 [0.881, 0.938] | 0.995 / 1.062 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | row_density |  | 0.375 [0.372, 0.378] | 0.383 [0.375, 0.390] | 0.419 / 0.454 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.086 / 0.090 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | fixed_schedule |  | 1.004 [0.989, 1.151] | 1.044 [0.990, 1.154] | 0.087 / 0.091 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_fused |  | 0.936 [0.914, 0.944] | 0.943 [0.907, 1.004] | 0.078 / 0.086 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_adaptive |  | 1.039 [1.021, 1.178] | 1.073 [1.019, 1.177] | 0.089 / 0.093 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | row_density |  | 0.747 [0.706, 0.792] | 0.750 [0.709, 0.805] | 0.062 / 0.071 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.585 / 3.931 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | fixed_schedule |  | 1.867 [1.728, 1.901] | 1.813 [1.711, 1.838] | 6.609 / 6.926 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_fused |  | 0.858 [0.848, 0.863] | 0.847 [0.825, 0.867] | 3.067 / 3.314 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_adaptive |  | 1.877 [1.745, 1.907] | 1.817 [1.713, 1.838] | 6.633 / 6.951 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | row_density |  | 1.473 [1.450, 1.482] | 1.434 [1.401, 1.452] | 5.221 / 5.729 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.106 / 0.114 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | fixed_schedule |  | 0.807 [0.796, 0.824] | 0.808 [0.798, 0.822] | 0.087 / 0.092 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_fused |  | 1.116 [1.108, 1.126] | 1.116 [1.091, 1.129] | 0.119 / 0.127 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_adaptive |  | 1.114 [1.108, 1.124] | 1.114 [1.101, 1.133] | 0.119 / 0.128 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | row_density |  | 0.647 [0.645, 0.655] | 0.657 [0.643, 0.665] | 0.069 / 0.075 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.266 / 8.794 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | fixed_schedule |  | 0.803 [0.794, 0.808] | 0.796 [0.791, 0.808] | 6.637 / 7.084 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_fused |  | 1.169 [1.158, 1.175] | 1.165 [1.151, 1.172] | 9.651 / 10.221 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_adaptive |  | 1.168 [1.162, 1.177] | 1.168 [1.150, 1.175] | 9.670 / 10.254 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | row_density |  | 0.673 [0.668, 0.679] | 0.676 [0.664, 0.686] | 5.585 / 5.980 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.079 / 0.084 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | fixed_schedule |  | 1.091 [1.082, 1.117] | 1.095 [1.084, 1.124] | 0.087 / 0.091 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_fused |  | 1.025 [1.012, 1.030] | 1.025 [1.017, 1.031] | 0.081 / 0.084 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_adaptive |  | 1.024 [1.020, 1.029] | 1.027 [1.021, 1.036] | 0.081 / 0.086 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | row_density |  | 0.620 [0.616, 0.627] | 0.633 [0.624, 0.637] | 0.049 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | production_zero_skip | yes | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.672 / 4.969 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | fixed_schedule |  | 1.418 [1.409, 1.426] | 1.410 [1.389, 1.430] | 6.621 / 6.938 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_fused |  | 1.051 [1.046, 1.057] | 1.049 [1.039, 1.066] | 4.904 / 5.219 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_adaptive |  | 1.047 [1.044, 1.059] | 1.043 [1.035, 1.053] | 4.909 / 5.215 | 0.078 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | row_density |  | 0.645 [0.643, 0.654] | 0.649 [0.643, 0.658] | 3.018 / 3.250 | 0.078 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.085 / 0.089 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n16 | fixed_schedule |  | 9.885 [9.838, 9.935] | 9.802 [9.716, 9.925] | 0.843 / 0.882 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n16 | public_fused |  | 0.860 [0.853, 0.864] | 0.867 [0.818, 0.874] | 0.073 / 0.078 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n16 | public_adaptive |  | 0.724 [0.715, 0.727] | 0.726 [0.711, 0.734] | 0.061 / 0.066 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n16 | row_density | yes | 0.241 [0.238, 0.243] | 0.244 [0.234, 0.252] | 0.021 / 0.022 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.985 / 5.267 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n1024 | fixed_schedule |  | 10.846 [10.763, 10.880] | 10.723 [10.580, 11.027] | 53.926 / 56.146 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n1024 | public_fused |  | 0.858 [0.851, 0.866] | 0.856 [0.849, 0.906] | 4.271 / 4.530 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n1024 | public_adaptive |  | 0.661 [0.655, 0.665] | 0.664 [0.651, 0.675] | 3.288 / 3.529 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n1024 | row_density | yes | 0.212 [0.211, 0.214] | 0.217 [0.211, 0.225] | 1.061 / 1.157 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.639 / 0.664 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n16 | fixed_schedule |  | 1.326 [1.305, 1.364] | 1.308 [1.287, 1.337] | 0.843 / 0.875 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n16 | public_fused |  | 0.997 [0.986, 1.002] | 1.005 [0.983, 1.016] | 0.636 / 0.668 | 0.002 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n16 | public_adaptive |  | 1.329 [1.313, 1.376] | 1.327 [1.299, 1.350] | 0.850 / 0.881 | 0.002 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n16 | row_density | yes | 0.546 [0.532, 0.577] | 0.556 [0.527, 0.582] | 0.358 / 0.379 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.928 / 35.836 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n1024 | fixed_schedule |  | 2.170 [1.896, 2.895] | 1.583 [1.469, 1.937] | 53.811 / 56.111 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n1024 | public_fused |  | 1.054 [0.884, 1.391] | 1.058 [0.968, 1.206] | 25.502 / 36.593 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n1024 | public_adaptive |  | 2.167 [1.895, 2.899] | 1.580 [1.475, 1.930] | 53.791 / 56.148 | 0.141 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n1024 | row_density | yes | 0.664 [0.541, 0.801] | 0.643 [0.593, 0.768] | 14.989 / 25.539 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.495 / 1.615 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n16 | fixed_schedule |  | 0.557 [0.552, 0.564] | 0.561 [0.529, 0.569] | 0.836 / 0.896 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n16 | public_fused |  | 0.970 [0.959, 0.982] | 0.954 [0.918, 0.983] | 1.454 / 1.518 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n16 | public_adaptive |  | 0.521 [0.515, 0.525] | 0.515 [0.488, 0.523] | 0.777 / 0.840 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n16 | row_density | yes | 0.423 [0.418, 0.425] | 0.425 [0.400, 0.432] | 0.631 / 0.687 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 125.954 / 132.108 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n1024 | fixed_schedule |  | 0.426 [0.423, 0.430] | 0.432 [0.420, 0.442] | 53.546 / 58.045 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n1024 | public_fused |  | 0.998 [0.990, 1.008] | 0.990 [0.977, 1.005] | 125.477 / 133.335 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n1024 | public_adaptive |  | 0.519 [0.514, 0.526] | 0.519 [0.507, 0.528] | 65.096 / 70.289 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n1024 | row_density | yes | 0.424 [0.421, 0.432] | 0.430 [0.415, 0.435] | 53.279 / 58.139 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.988 / 1.051 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n16 | fixed_schedule |  | 0.844 [0.837, 0.853] | 0.844 [0.829, 0.856] | 0.835 / 0.895 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n16 | public_fused |  | 0.984 [0.975, 0.997] | 0.981 [0.966, 0.997] | 0.975 / 1.022 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n16 | public_adaptive |  | 0.535 [0.531, 0.544] | 0.542 [0.534, 0.549] | 0.534 / 0.569 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n16 | row_density | yes | 0.421 [0.416, 0.427] | 0.426 [0.417, 0.431] | 0.419 / 0.443 | 0.002 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n1024 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 63.608 / 79.472 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n1024 | fixed_schedule |  | 0.852 [0.843, 0.858] | 0.849 [0.504, 0.866] | 54.153 / 66.080 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n1024 | public_fused |  | 1.000 [0.992, 1.014] | 1.002 [0.760, 1.062] | 63.877 / 74.159 | 0.141 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n1024 | public_adaptive |  | 0.539 [0.533, 0.542] | 0.559 [0.517, 0.730] | 34.160 / 42.738 | 0.141 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n1024 | row_density | yes | 0.426 [0.422, 0.430] | 0.424 [0.304, 0.435] | 27.021 / 33.935 | 0.141 | 0 / 0 | pass |
