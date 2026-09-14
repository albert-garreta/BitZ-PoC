# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_integer_mac | l1_signed16_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n16 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n16 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n16 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n16 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.335 / 0.347 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | products2 |  | 1.011 [1.007, 1.016] | 1.007 [0.997, 1.020] | 0.338 / 0.349 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | products4 |  | 0.998 [0.996, 1.005] | 1.001 [0.983, 1.019] | 0.334 / 0.348 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | deferred_columns |  | 1.001 [0.998, 1.005] | 1.005 [0.991, 1.016] | 0.335 / 0.349 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | incumbent | yes | 1.005 [0.999, 1.010] | 1.002 [0.988, 1.014] | 0.336 / 0.347 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.335 / 0.347 | 0.016 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n1024 | products2 |  | 1.013 [1.006, 1.017] | 1.017 [0.999, 1.031] | 0.339 / 0.352 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n1024 | products4 |  | 0.997 [0.994, 1.003] | 1.002 [0.993, 1.020] | 0.334 / 0.348 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n1024 | deferred_columns |  | 0.997 [0.990, 1.003] | 1.012 [0.984, 1.020] | 0.334 / 0.349 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n1024 | incumbent | yes | 1.003 [0.997, 1.012] | 1.010 [0.994, 1.022] | 0.336 / 0.350 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.239 / 22.167 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n65536 | products2 |  | 0.998 [0.993, 1.006] | 0.990 [0.979, 1.004] | 21.196 / 21.934 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n65536 | products4 |  | 0.997 [0.992, 1.004] | 0.991 [0.974, 1.006] | 21.164 / 21.988 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l1_full_n65536 | deferred_columns |  | 0.998 [0.992, 1.005] | 0.992 [0.981, 1.015] | 21.214 / 22.056 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_integer_mac | l1_full_n65536 | incumbent | yes | 0.999 [0.991, 1.004] | 0.995 [0.976, 1.006] | 21.163 / 21.928 | 1.000 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l2_signed16_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n16 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n16 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n16 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n16 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n1024 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_signed16_n65536 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n16 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n16 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n16 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n16 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n1024 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n1024 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n1024 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n1024 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n65536 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n65536 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n65536 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l2_full_n65536 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n16 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n16 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n16 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n16 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n1024 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_signed16_n65536 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n16 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n16 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n16 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n16 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n1024 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n1024 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n1024 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n1024 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n65536 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n65536 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n65536 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l9_full_n65536 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n16 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n16 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n16 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n16 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n16 | native128x2 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n16 | native128x2_acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | native128x2 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n1024 | native128x2_acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | native128x2 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_signed16_n65536 | native128x2_acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n16 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n16 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n16 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n16 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n16 | native128x2 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n16 | native128x2_acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n1024 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n1024 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n1024 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n1024 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n1024 | native128x2 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n1024 | native128x2_acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n65536 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n65536 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n65536 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n65536 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n65536 | native128x2 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l4_full_n65536 | native128x2_acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l2_n16 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l2_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l2_n16 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l2_n16 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l2_n1024 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l2_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l2_n1024 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l2_n1024 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l2_n16 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.312 / 0.357 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n16 | public_divisor |  | 2.487 [2.206, 3.007] | 2.440 [2.180, 2.901] | 0.775 / 0.791 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_projection | q128_l2_n16 | horner_prepared | yes | 0.416 [0.357, 0.482] | 0.421 [0.363, 0.477] | 0.125 / 0.134 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_projection | q128_l2_n16 | horner_one_shot | yes | 0.888 [0.782, 1.070] | 0.881 [0.784, 1.061] | 0.276 / 0.287 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_projection | q128_l2_n1024 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l2_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l2_n1024 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l2_n1024 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l4_n16 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l4_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l4_n16 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l4_n16 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l4_n1024 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l4_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l4_n1024 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l4_n1024 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l4_n16 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l4_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l4_n16 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l4_n16 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l4_n1024 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l4_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l4_n1024 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l4_n1024 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l9_n16 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l9_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l9_n16 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l9_n16 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l9_n1024 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l9_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l9_n1024 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q100_l9_n1024 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l9_n16 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l9_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l9_n16 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l9_n16 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l9_n1024 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l9_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l9_n1024 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l9_n1024 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q100_l2 | horner |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q128_l2 | horner |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q100_l4 | horner |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q128_l4 | horner |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q100_l9 | horner |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q128_l9 | horner |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l2_n16 | generic |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l2_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l2_n16 | prepared_reciprocal | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l2_n16 | prepare_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l2_n1024 | generic |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l2_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l2_n1024 | prepared_reciprocal | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l2_n1024 | prepare_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l4_n16 | generic |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l4_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l4_n16 | prepared_reciprocal | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l4_n16 | prepare_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l4_n1024 | generic |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l4_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l4_n1024 | prepared_reciprocal | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l4_n1024 | prepare_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l9_n16 | generic |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l9_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l9_n16 | prepared_reciprocal | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l9_n16 | prepare_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l9_n1024 | generic |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l9_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l9_n1024 | prepared_reciprocal | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l9_n1024 | prepare_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l32_n16 | generic |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l32_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l32_n16 | prepared_reciprocal | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l32_n16 | prepare_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l32_n1024 | generic |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l32_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l32_n1024 | prepared_reciprocal | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l32_n1024 | prepare_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l64_n16 | generic |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l64_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l64_n16 | prepared_reciprocal | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l64_n16 | prepare_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l64_n1024 | generic |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l64_n1024 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l64_n1024 | prepared_reciprocal | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_divrem64 | l64_n1024 | prepare_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l1_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l1_n16 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l1_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.215 / 0.225 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_add | l1_n1024 | word_carry | yes | 1.000 [0.995, 1.005] | 1.001 [0.987, 1.012] | 0.215 / 0.224 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_add | l2_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l2_n16 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l2_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l2_n1024 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l4_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l4_n16 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l4_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l4_n1024 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l9_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l9_n16 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l9_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l9_n1024 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l1_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l1_n16 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l1_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.214 / 0.227 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l1_n1024 | word_carry | yes | 0.999 [0.993, 1.007] | 0.985 [0.965, 1.013] | 0.214 / 0.224 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l2_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n16 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.671 / 0.694 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l2_n1024 | word_carry | yes | 0.999 [0.994, 1.004] | 0.998 [0.978, 1.082] | 0.670 / 0.725 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l4_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.022 / 0.023 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l4_n16 | word_carry | yes | 1.008 [0.995, 1.158] | 1.008 [0.980, 1.149] | 0.022 / 0.026 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l4_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.360 / 1.411 | 0.094 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l4_n1024 | word_carry | yes | 1.006 [0.999, 1.011] | 1.011 [0.992, 1.027] | 1.366 / 1.436 | 0.094 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l9_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.064 / 0.067 | 0.003 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l9_n16 | word_carry | yes | 1.009 [0.989, 1.017] | 1.000 [0.983, 1.024] | 0.064 / 0.068 | 0.003 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_sub | l9_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.003 / 4.177 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_sub | l9_n1024 | word_carry | yes | 0.989 [0.975, 0.993] | 0.980 [0.968, 1.000] | 3.938 / 4.136 | 0.211 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.493 / 0.515 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_add | l1_n1024 | word_option | yes | 1.007 [0.958, 1.011] | 0.992 [0.961, 1.026] | 0.494 / 0.517 | 0.023 | 0 / 0 | inconclusive |
| aarch64 | opt_uint_checked_add | l2_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.203 / 1.263 | 0.047 | 0 / 0 | pass |
| aarch64 | opt_uint_checked_sub | l2_n1024 | word_option | yes | 1.017 [1.012, 1.022] | 1.014 [0.993, 1.039] | 1.224 / 1.286 | 0.047 | 0 / 0 | regression |
| aarch64 | opt_uint_checked_sub | l4_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l1_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l1_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l1_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l1_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l2_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l2_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l2_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l2_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l4_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l4_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l4_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l4_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l9_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l9_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l9_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l9_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l1_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l1_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l1_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l1_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l2_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l2_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l2_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l2_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l4_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l4_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l4_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l4_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l9_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l9_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l9_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l9_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a1_b1_n16 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a1_b1_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a1_b1_n1024 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a1_b1_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b2_n16 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b2_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b2_n1024 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b2_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a4_b4_n16 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a4_b4_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a4_b4_n1024 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a4_b4_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a9_b9_n16 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a9_b9_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a9_b9_n1024 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a9_b9_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b9_n16 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b9_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b9_n1024 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b9_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a32_b32_n16 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a32_b32_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a32_b32_n1024 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a32_b32_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a64_b64_n16 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a64_b64_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a64_b64_n1024 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a64_b64_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n16 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n1024 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n16 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n1024 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n16 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n1024 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l2_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l2_n16 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l2_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l2_n1024 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l4_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l4_n16 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l4_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l4_n1024 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l9_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l9_n16 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l9_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l9_n1024 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n16 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.061 / 0.065 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q100_n16 | acc4 |  | 1.046 [1.040, 1.059] | 1.041 [1.025, 1.062] | 0.063 / 0.069 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_prime_dot | q100_n16 | length_dispatch | yes | 1.005 [0.997, 1.014] | 1.006 [0.985, 1.025] | 0.061 / 0.066 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_dot | q100_n1024 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n1024 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n1024 | length_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n65536 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n65536 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n65536 | length_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n16 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.060 / 0.064 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_dot | q128_n16 | acc4 |  | 1.046 [1.041, 1.052] | 1.052 [1.038, 1.164] | 0.063 / 0.070 | 0.000 | 0 / 0 | regression |
| aarch64 | opt_prime_dot | q128_n16 | length_dispatch | yes | 1.000 [0.997, 1.008] | 1.007 [0.992, 1.037] | 0.060 / 0.065 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_dot | q128_n1024 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n1024 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n1024 | length_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n65536 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n65536 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n65536 | length_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n16 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.042 / 0.049 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q100_n16 | acc4 | yes | 0.995 [0.987, 1.011] | 0.986 [0.960, 1.022] | 0.041 / 0.049 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_linear | q100_n1024 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n65536 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n65536 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n16 | one_acc |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.042 / 0.044 | 0.000 | 0 / 0 | pass |
| aarch64 | opt_prime_linear | q128_n16 | acc4 | yes | 0.995 [0.992, 1.002] | 0.997 [0.985, 1.014] | 0.041 / 0.043 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | opt_prime_linear | q128_n1024 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n65536 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n65536 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n16 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n16 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_public_pow | e17_n16 | bounded_window |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_public_pow | e17_n16 | public_binary | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_public_pow | e17_n1024 | bounded_window |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_public_pow | e17_n1024 | public_binary | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_public_pow | e127_n16 | bounded_window |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_public_pow | e127_n16 | public_binary | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_public_pow | e127_n1024 | bounded_window |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_public_pow | e127_n1024 | public_binary | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | zero_n16 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | zero_n16 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | zero_n16 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | zero_n1024 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | zero_n1024 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | zero_n1024 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | zero_n65536 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | zero_n65536 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | zero_n65536 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | half_n16 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | half_n16 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | half_n16 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | half_n1024 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | half_n1024 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | half_n1024 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | half_n65536 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | half_n65536 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | half_n65536 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n16 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n16 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n16 | public_scalar_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n1024 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n1024 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n1024 | public_scalar_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n65536 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n65536 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n65536 | public_scalar_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | zero_n16 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | zero_n16 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | zero_n1024 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | zero_n1024 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | zero_n65536 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | zero_n65536 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | half_n16 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | half_n16 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | half_n1024 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | half_n1024 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | half_n65536 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | half_n65536 | public_scalar_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | full_n16 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | full_n16 | public_scalar_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | full_n1024 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | full_n1024 | public_scalar_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | full_n65536 | actual_fixed_gf |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | full_n65536 | public_scalar_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_round | 16 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_round | 16 | wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_round | 16 | wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_round | 1024 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_round | 1024 | wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_round | 1024 | wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_round | 65536 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_round | 65536 | wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_round | 65536 | wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_mul | 16 | flock_scalar |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_mul | 16 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_mul | 1024 | flock_scalar |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_mul | 1024 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_mul | 65536 | flock_scalar |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_mul | 65536 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 16 | table_public_input |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 16 | fixed_basis |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 1024 | table_public_input |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 1024 | fixed_basis |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 65536 | table_public_input |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 65536 | fixed_basis |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 16 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 16 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 16 | pfold |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 1024 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 1024 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 1024 | pfold |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 65536 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 65536 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 65536 | pfold |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | production_blocked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | reuse_products | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | production_blocked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | reuse_products | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | production_blocked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | reuse_products | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log10 | allocate |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log10 | scratch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log16 | allocate |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log16 | scratch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log18 | allocate |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 204.674 / 229.757 | 4.000 | 2 / 66560 | pass |
| aarch64 | opt_ood_reuse | log18 | scratch | yes | 0.987 [0.977, 1.004] | 0.997 [0.962, 1.031] | 204.150 / 226.594 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log8_lanes32 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | depth_first | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | depth_first | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3872.291 / 4068.587 | 12.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log15_lanes8 | depth_first | yes | 0.936 [0.930, 0.943] | 0.947 [0.913, 0.969] | 3628.166 / 3845.724 | 12.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log15_lanes32 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes32 | depth_first | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log16_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 24058.021 / 24832.033 | 96.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log16_lanes32 | depth_first | yes | 0.957 [0.951, 0.973] | 0.989 [0.954, 1.068] | 23060.854 / 24685.844 | 96.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 107818.708 / 110571.215 | 384.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log18_lanes32 | depth_first | yes | 0.963 [0.954, 0.965] | 0.964 [0.954, 0.973] | 103932.333 / 106501.519 | 384.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log8_lanes1 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs9_9 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs9_9 | single_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs16_14 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs16_14 | single_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs18_18 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs18_18 | single_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs9_9 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs9_9 | single_write_ood | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs16_14 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 271.453 / 294.202 | 4.000 | 21 / 2229728 | pass |
| aarch64 | opt_packed_ood | logs16_14 | single_write_ood | yes | 1.114 [1.103, 1.123] | 1.073 [1.057, 1.105] | 301.917 / 316.224 | 4.000 | 3 / 2163200 | regression |
| aarch64 | opt_packed_ood | logs18_18 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs18_18 | single_write_ood | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_two_pair | 16 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_two_pair | 16 | separate_wide | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_two_pair | 1024 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_two_pair | 1024 | separate_wide | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fold_round | 16 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fold_round | 16 | fold_then_wide | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fold_round | 1024 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fold_round | 1024 | fold_then_wide | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 16 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 16 | chunked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 1024 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 1024 | chunked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 16 | flock_scalar |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 16 | vector_chain | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 1024 | flock_scalar |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 1024 | vector_chain | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 65536 | flock_scalar |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 65536 | vector_chain | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n16 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_n1024 | fixed_schedule | yes | — | — | — | — | — | unmeasured |
