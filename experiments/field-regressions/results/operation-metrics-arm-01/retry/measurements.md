# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | metrics_gf_mul | 16 | f2z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_gf_mul | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_gf_mul | 16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_gf_mul | 16 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | metrics_gf_mul | 1024 | f2z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_gf_mul | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_gf_mul | 1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_gf_mul | 1024 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | gf8_add | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | gf8_add | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | gf8_mul | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | gf8_mul | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | gf8_inverse | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | gf8_inverse | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | b127_add | 16 | f2z |  | — | — | — | — | — | unmeasured |
| aarch64 | b127_add | 1024 | f2z |  | — | — | — | — | — | unmeasured |
| aarch64 | b127_mul | 16 | f2z |  | — | — | — | — | — | unmeasured |
| aarch64 | b127_mul | 1024 | f2z |  | — | — | — | — | — | unmeasured |
| aarch64 | b127_square | 16 | f2z |  | — | — | — | — | — | unmeasured |
| aarch64 | b127_square | 1024 | f2z |  | — | — | — | — | — | unmeasured |
| aarch64 | b127_inverse | 16 | f2z |  | — | — | — | — | — | unmeasured |
| aarch64 | b127_inverse | 1024 | f2z |  | — | — | — | — | — | unmeasured |
| aarch64 | gf128_round | 16 | f2z_single_pair |  | — | — | — | — | — | unmeasured |
| aarch64 | gf128_round | 1024 | f2z_single_pair |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_add | l2_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_add | l2_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_add | l4_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_add | l4_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_add | l9_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_add | l9_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_sub | l2_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_sub | l2_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_sub | l4_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_sub | l4_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_sub | l9_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_integer_sub | l9_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_add | l2_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_add | l2_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_add | l4_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_add | l4_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_add | l9_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_add | l9_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_sub | l2_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_sub | l2_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_sub | l4_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_sub | l4_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_sub | l9_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_sub | l9_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_mul | l2_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_mul | l2_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_mul | l4_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_mul | l4_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_mul | l9_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_checked_mul | l9_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_add | l2_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_add | l2_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_add | l4_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_add | l4_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_add | l9_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_add | l9_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_sub | l2_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_sub | l2_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_sub | l4_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_sub | l4_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_sub | l9_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_sub | l9_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_mul | l2_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_mul | l2_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_mul | l4_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_mul | l4_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_mul | l9_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_int_checked_mul | l9_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_compare | l2_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_compare | l2_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_compare | l4_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_compare | l4_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_compare | l9_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_compare | l9_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_wide_mul | l2_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_wide_mul | l2_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_wide_mul | l4_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_wide_mul | l4_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_wide_mul | l9_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_wide_mul | l9_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l2_d64_n16 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l2_dfull_n16 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l2_d64_n1024 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l2_dfull_n1024 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l4_d64_n16 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l4_dfull_n16 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l4_d64_n1024 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l4_dfull_n1024 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l9_d64_n16 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l9_dfull_n16 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l9_d64_n1024 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | metrics_uint_divrem | l9_dfull_n1024 | existing_prevalidated_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_context_setup | q127 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_inverse_each | q127_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_inverse_each | q127_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_canonical_encode_into | q127_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_canonical_encode_into | q127_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_canonical_decode_into | q127_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_canonical_decode_into | q127_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_pow_each | q127_e17_sparse_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_pow_each | q127_e127_alternating_n16 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_pow_each | q127_e17_sparse_n1024 | existing |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_pow_each | q127_e127_alternating_n1024 | existing |  | — | — | — | — | — | unmeasured |
