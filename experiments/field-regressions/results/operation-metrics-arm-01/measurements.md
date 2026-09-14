# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | metrics_gf_mul | 16 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_gf_mul | 16 | flock |  | 0.970 [0.966, 0.977] | 0.968 [0.959, 0.982] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_gf_mul | 16 | shared |  | 1.001 [0.996, 1.005] | 1.000 [0.990, 1.007] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_gf_mul | 16 | scalar_lanes | yes | 0.974 [0.970, 0.982] | 0.978 [0.965, 1.000] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_gf_mul | 1024 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.928 / 0.963 | 0.047 | 0 / 0 | pass |
| aarch64 | metrics_gf_mul | 1024 | flock |  | 0.937 [0.933, 0.945] | 0.932 [0.907, 0.959] | 0.872 / 0.904 | 0.047 | 0 / 0 | pass |
| aarch64 | metrics_gf_mul | 1024 | shared |  | 1.001 [0.996, 1.011] | 0.989 [0.965, 1.014] | 0.929 / 0.962 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | metrics_gf_mul | 1024 | scalar_lanes | yes | 0.943 [0.935, 0.950] | 0.933 [0.907, 0.952] | 0.874 / 0.903 | 0.047 | 0 / 0 | pass |
| aarch64 | metrics_gf_add | 16 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.010 / 0.011 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_gf_add | 16 | flock |  | 0.980 [0.936, 1.015] | 0.984 [0.940, 1.019] | 0.010 / 0.011 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | metrics_gf_add | 16 | shared | yes | 0.996 [0.960, 1.016] | 0.988 [0.954, 1.032] | 0.010 / 0.011 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | metrics_gf_add | 1024 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.277 / 0.335 | 0.047 | 0 / 0 | pass |
| aarch64 | metrics_gf_add | 1024 | flock |  | 0.999 [0.991, 1.023] | 0.998 [0.986, 1.031] | 0.279 / 0.335 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | metrics_gf_add | 1024 | shared | yes | 0.998 [0.991, 1.003] | 0.997 [0.980, 1.010] | 0.277 / 0.332 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | metrics_gf_square | 16 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | metrics_gf_square | 16 | flock |  | 3.370 [3.352, 3.380] | 3.334 [3.068, 3.375] | 0.040 / 0.043 | 0.000 | 0 / 0 | regression |
| aarch64 | metrics_gf_square | 16 | shared | yes | 1.002 [0.995, 1.007] | 0.994 [0.939, 1.018] | 0.012 / 0.013 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | metrics_gf_square | 1024 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.673 / 0.841 | 0.031 | 0 / 0 | pass |
| aarch64 | metrics_gf_square | 1024 | flock |  | 3.628 [3.618, 3.661] | 3.614 [3.449, 4.747] | 2.436 / 3.194 | 0.031 | 0 / 0 | regression |
| aarch64 | metrics_gf_square | 1024 | shared | yes | 1.007 [0.996, 1.011] | 1.011 [0.933, 1.094] | 0.673 / 0.818 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | metrics_gf_inverse | 16 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 7.135 / 7.644 | 0.000 | 0 / 0 | pass |
| aarch64 | metrics_gf_inverse | 16 | flock |  | 2.429 [2.418, 2.441] | 2.410 [2.349, 2.443] | 17.307 / 18.439 | 0.000 | 0 / 0 | regression |
| aarch64 | metrics_gf_inverse | 16 | shared | yes | 0.998 [0.992, 1.004] | 1.002 [0.969, 1.015] | 7.133 / 7.589 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | metrics_gf_inverse | 1024 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 457.521 / 494.666 | 0.031 | 0 / 0 | pass |
| aarch64 | metrics_gf_inverse | 1024 | flock |  | 2.428 [2.416, 2.441] | 2.389 [2.342, 2.486] | 1109.026 / 1200.888 | 0.031 | 0 / 0 | regression |
| aarch64 | metrics_gf_inverse | 1024 | shared | yes | 0.998 [0.993, 1.004] | 0.996 [0.956, 1.016] | 457.357 / 495.703 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | gf8_add | 16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | gf8_add | 1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.023 | 0.003 | 0 / 0 | pass |
| aarch64 | gf8_mul | 16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.024 / 0.025 | 0.000 | 0 / 0 | pass |
| aarch64 | gf8_mul | 1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.488 / 1.565 | 0.003 | 0 / 0 | pass |
| aarch64 | gf8_inverse | 16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.584 / 0.597 | 0.000 | 0 / 0 | pass |
| aarch64 | gf8_inverse | 1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 37.277 / 39.252 | 0.002 | 0 / 0 | pass |
| aarch64 | b127_add | 16 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.010 / 0.010 | 0.001 | 0 / 0 | pass |
| aarch64 | b127_add | 1024 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.277 / 0.382 | 0.047 | 0 / 0 | pass |
| aarch64 | b127_mul | 16 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.020 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | b127_mul | 1024 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.234 / 1.314 | 0.047 | 0 / 0 | pass |
| aarch64 | b127_square | 16 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | b127_square | 1024 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.665 / 0.685 | 0.031 | 0 / 0 | pass |
| aarch64 | b127_inverse | 16 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6.859 / 7.054 | 0.000 | 0 / 0 | pass |
| aarch64 | b127_inverse | 1024 | f2z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 436.935 / 457.311 | 0.031 | 0 / 0 | pass |
| aarch64 | gf128_round | 16 | f2z_single_pair |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.093 / 0.095 | 0.001 | 0 / 0 | pass |
| aarch64 | gf128_round | 1024 | f2z_single_pair |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 5.598 / 5.702 | 0.078 | 0 / 0 | pass |
| aarch64 | metrics_integer_add | l2_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_integer_add | l2_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.631 / 0.655 | 0.047 | 0 / 0 | pass |
| aarch64 | metrics_integer_add | l4_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.033 / 0.035 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_integer_add | l4_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.010 / 2.189 | 0.094 | 0 / 0 | pass |
| aarch64 | metrics_integer_add | l9_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.055 / 0.057 | 0.003 | 0 / 0 | pass |
| aarch64 | metrics_integer_add | l9_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.731 / 3.854 | 0.211 | 0 / 0 | pass |
| aarch64 | metrics_integer_sub | l2_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_integer_sub | l2_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.668 / 0.833 | 0.047 | 0 / 0 | pass |
| aarch64 | metrics_integer_sub | l4_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.028 / 0.029 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_integer_sub | l4_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.658 / 1.714 | 0.094 | 0 / 0 | pass |
| aarch64 | metrics_integer_sub | l9_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.060 / 0.062 | 0.003 | 0 / 0 | pass |
| aarch64 | metrics_integer_sub | l9_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.962 / 4.094 | 0.211 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_add | l2_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.022 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_add | l2_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.255 / 1.295 | 0.055 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_add | l4_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.025 / 0.026 | 0.002 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_add | l4_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.505 / 1.562 | 0.102 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_add | l9_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.059 / 0.061 | 0.003 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_add | l9_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.346 / 4.481 | 0.219 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_sub | l2_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.037 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_sub | l2_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.243 / 1.280 | 0.055 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_sub | l4_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.028 / 0.030 | 0.002 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_sub | l4_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.721 / 1.843 | 0.102 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_sub | l9_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.074 / 0.077 | 0.003 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_sub | l9_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.725 / 4.852 | 0.219 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_mul | l2_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.033 / 0.035 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_mul | l2_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.959 / 2.088 | 0.055 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_mul | l4_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.119 / 0.122 | 0.002 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_mul | l4_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 7.511 / 7.800 | 0.102 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_mul | l9_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.949 / 1.000 | 0.003 | 0 / 0 | pass |
| aarch64 | metrics_uint_checked_mul | l9_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 62.452 / 64.963 | 0.219 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_add | l2_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.020 / 0.065 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_add | l2_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.161 / 1.367 | 0.055 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_add | l4_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.026 / 0.027 | 0.002 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_add | l4_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.035 / 2.237 | 0.102 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_add | l9_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.072 / 0.081 | 0.003 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_add | l9_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.632 / 4.768 | 0.219 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_sub | l2_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.020 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_sub | l2_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.205 / 1.588 | 0.055 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_sub | l4_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.037 / 0.038 | 0.002 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_sub | l4_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.335 / 2.459 | 0.102 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_sub | l9_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.101 / 0.109 | 0.003 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_sub | l9_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6.529 / 6.911 | 0.219 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_mul | l2_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.063 / 0.065 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_mul | l2_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.910 / 4.177 | 0.055 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_mul | l4_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.156 / 0.160 | 0.002 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_mul | l4_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 9.934 / 10.180 | 0.102 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_mul | l9_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.499 / 0.516 | 0.003 | 0 / 0 | pass |
| aarch64 | metrics_int_checked_mul | l9_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 32.026 / 33.594 | 0.219 | 0 / 0 | pass |
| aarch64 | metrics_uint_compare | l2_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.015 / 0.016 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_uint_compare | l2_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.779 / 0.817 | 0.032 | 0 / 0 | pass |
| aarch64 | metrics_uint_compare | l4_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.034 / 0.037 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_uint_compare | l4_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.004 / 2.114 | 0.063 | 0 / 0 | pass |
| aarch64 | metrics_uint_compare | l9_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.098 / 0.108 | 0.002 | 0 / 0 | pass |
| aarch64 | metrics_uint_compare | l9_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6.269 / 6.518 | 0.142 | 0 / 0 | pass |
| aarch64 | metrics_uint_wide_mul | l2_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.024 / 0.025 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_uint_wide_mul | l2_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.425 / 1.453 | 0.062 | 0 / 0 | pass |
| aarch64 | metrics_uint_wide_mul | l4_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.108 / 0.116 | 0.002 | 0 / 0 | pass |
| aarch64 | metrics_uint_wide_mul | l4_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6.795 / 7.187 | 0.125 | 0 / 0 | pass |
| aarch64 | metrics_uint_wide_mul | l9_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.794 / 0.823 | 0.004 | 0 / 0 | pass |
| aarch64 | metrics_uint_wide_mul | l9_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 50.714 / 52.602 | 0.281 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l2_d64_n16 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.716 / 0.734 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l2_dfull_n16 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.723 / 0.757 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l2_d64_n1024 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 44.921 / 46.158 | 0.047 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l2_dfull_n1024 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 45.032 / 46.463 | 0.047 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l4_d64_n16 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.500 / 1.570 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l4_dfull_n16 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.510 / 1.565 | 0.001 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l4_d64_n1024 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 95.417 / 98.982 | 0.094 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l4_dfull_n1024 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 96.247 / 100.152 | 0.094 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l9_d64_n16 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.291 / 4.437 | 0.003 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l9_dfull_n16 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.336 / 4.470 | 0.003 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l9_d64_n1024 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 273.624 / 281.308 | 0.211 | 0 / 0 | pass |
| aarch64 | metrics_uint_divrem | l9_dfull_n1024 | existing_prevalidated_divisor |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 276.236 / 282.515 | 0.211 | 0 / 0 | pass |
| aarch64 | prime_context_setup | q127 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.122 / 0.126 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_inverse_each | q127_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 13.611 / 14.126 | 0.002 | 0 / 0 | pass |
| aarch64 | prime_inverse_each | q127_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 872.979 / 898.836 | 0.156 | 0 / 0 | pass |
| aarch64 | prime_canonical_encode_into | q127_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.034 / 0.035 | 0.001 | 0 / 0 | pass |
| aarch64 | prime_canonical_encode_into | q127_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.117 / 2.193 | 0.094 | 0 / 0 | pass |
| aarch64 | prime_canonical_decode_into | q127_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.093 / 0.097 | 0.001 | 0 / 0 | pass |
| aarch64 | prime_canonical_decode_into | q127_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 5.216 / 5.420 | 0.094 | 0 / 0 | pass |
| aarch64 | prime_pow_each | q127_e17_sparse_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6.689 / 6.950 | 0.002 | 0 / 0 | pass |
| aarch64 | prime_pow_each | q127_e127_alternating_n16 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 30.951 / 32.365 | 0.002 | 0 / 0 | pass |
| aarch64 | prime_pow_each | q127_e17_sparse_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 429.523 / 444.950 | 0.156 | 0 / 0 | pass |
| aarch64 | prime_pow_each | q127_e127_alternating_n1024 | existing |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1991.136 / 2070.302 | 0.156 | 0 / 0 | pass |
