# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_integer_mac | l1_signed16_n16 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n16 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n16 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n16 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n16 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n16 | native_word |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | native_word |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n65536 | native_word |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n16 | native_word |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | native_word |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | incumbent |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | native_word |  | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_integer_mac | l4_full_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.054 / 0.057 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n16 | products2 |  | 1.072 [1.065, 1.100] | 1.073 [1.057, 1.097] | 0.058 / 0.061 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | products4 |  | 1.113 [1.110, 1.125] | 1.114 [1.078, 1.150] | 0.060 / 0.063 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | deferred_columns |  | 1.273 [1.247, 1.276] | 1.259 [1.199, 1.278] | 0.068 / 0.071 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | incumbent |  | 1.086 [1.076, 1.091] | 1.072 [1.058, 1.096] | 0.058 / 0.061 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_integer_mac | l4_full_n16 | native128x2 | yes | 0.920 [0.915, 0.925] | 0.911 [0.894, 0.933] | 0.049 / 0.052 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_integer_mac | l4_full_n16 | native128x2_acc2 |  | 0.918 [0.911, 0.925] | 0.922 [0.905, 0.978] | 0.049 / 0.052 | 0.001 | 0 / 0 | pass |
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
| aarch64 | opt_projection | q128_l2_n16 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l2_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l2_n16 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l2_n16 | horner_one_shot |  | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_projection_setup | q100_l2 | horner | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q128_l2 | horner | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q100_l4 | horner | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q128_l4 | horner | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q100_l9 | horner | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection_setup | q128_l9 | horner | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_uint_add | l1_n16 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l1_n16 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l1_n16 | native_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l1_n1024 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l1_n1024 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l1_n1024 | native_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l2_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l2_n16 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l2_n16 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l2_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l2_n1024 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l2_n1024 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l4_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l4_n16 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l4_n16 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l4_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l4_n1024 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l4_n1024 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l9_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l9_n16 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l9_n16 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l9_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l9_n1024 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l9_n1024 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l1_n16 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l1_n16 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l1_n16 | native_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l1_n1024 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l1_n1024 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l1_n1024 | native_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n16 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n16 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n1024 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n1024 | native_batch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l4_n16 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l4_n16 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l4_n16 | native_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l4_n1024 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l4_n1024 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l4_n1024 | native_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l9_n16 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l9_n16 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l9_n16 | native_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l9_n1024 | circuit_z | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l9_n1024 | word_carry |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l9_n1024 | native_batch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n16 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n16 | limb_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n1024 | vendor_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n1024 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n1024 | limb_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n16 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n16 | limb_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n1024 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l2_n1024 | limb_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n16 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n16 | limb_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n1024 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l4_n1024 | limb_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n16 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n16 | limb_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n1024 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l9_n1024 | limb_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n16 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n16 | native_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n16 | limb_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n1024 | vendor_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n1024 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l1_n1024 | limb_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n16 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n16 | native_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n16 | limb_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n1024 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n1024 | native_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n1024 | limb_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n16 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n16 | native_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n16 | limb_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n1024 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n1024 | native_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l4_n1024 | limb_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n16 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n16 | native_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n16 | limb_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n1024 | word_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n1024 | native_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l9_n1024 | limb_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l1_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l1_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l1_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l1_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l1_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l1_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l2_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l2_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l2_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l2_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l2_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l2_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l4_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l4_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l4_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l4_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l4_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l4_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l9_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l9_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l9_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l9_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l9_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_add | l9_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l1_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l1_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l1_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l1_n1024 | vendor_option |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.078 / 1.457 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n1024 | word_option | yes | 0.673 [0.623, 0.685] | 0.533 [0.493, 0.592] | 0.707 / 0.757 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l1_n1024 | native_option |  | 0.671 [0.622, 0.684] | 0.540 [0.489, 0.754] | 0.706 / 0.755 | 0.023 | 0 / 0 | pass |
| aarch64 | opt_int_checked_sub | l2_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l2_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l2_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l2_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l2_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l2_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l4_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l4_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l4_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l4_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l4_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l4_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l9_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l9_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l9_n16 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l9_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l9_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_int_checked_sub | l9_n1024 | native_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a1_b1_n16 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a1_b1_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a1_b1_n1024 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a1_b1_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b2_n16 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b2_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b2_n1024 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b2_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a4_b4_n16 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a4_b4_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a4_b4_n1024 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a4_b4_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a9_b9_n16 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a9_b9_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a9_b9_n1024 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a9_b9_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b9_n16 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b9_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b9_n1024 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a2_b9_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a32_b32_n16 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a32_b32_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a32_b32_n1024 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a32_b32_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a64_b64_n16 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a64_b64_n16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a64_b64_n1024 | crypto_bigint | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_product | a64_b64_n1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n16 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n1024 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n16 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n1024 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n16 | wide_then_add | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n16 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n1024 | wide_then_add | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n1024 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l2_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l2_n16 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l2_n16 | column_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l2_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l2_n1024 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l2_n1024 | column_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l4_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l4_n16 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l4_n16 | column_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l4_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l4_n1024 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l4_n1024 | column_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l9_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l9_n16 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l9_n16 | column_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l9_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l9_n1024 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_unsigned_mac | l9_n1024 | column_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n16 | one_acc | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n16 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n16 | length_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n1024 | one_acc | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n1024 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n1024 | length_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n65536 | one_acc | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n65536 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n65536 | length_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n16 | one_acc | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n16 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n16 | length_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n1024 | one_acc | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n1024 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n1024 | length_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n65536 | one_acc | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n65536 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n65536 | length_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n16 | one_acc | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n16 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n1024 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n65536 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n65536 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n16 | one_acc | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n16 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n1024 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n65536 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n65536 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n16 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n16 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n16 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | production_one_inverse | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n16 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n1024 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_nonzero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n16 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n1024 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_mixed_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n16 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n1024 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q100_zero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n16 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n1024 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_nonzero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n16 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n1024 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_mixed_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n16 | compact_vartime | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n16 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse_reuse | q128_zero_n1024 | compact_vartime | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_gf_fixed | full_n16 | actual_fixed_gf | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n16 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n16 | public_scalar_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n1024 | actual_fixed_gf | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n1024 | prepared_formula |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n1024 | public_scalar_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fixed | full_n65536 | actual_fixed_gf | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_gf_butterfly | full_n16 | actual_fixed_gf | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | full_n16 | public_scalar_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | full_n1024 | actual_fixed_gf | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | full_n1024 | public_scalar_dispatch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_butterfly | full_n65536 | actual_fixed_gf | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_phi8 | 16 | table_public_input | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 16 | fixed_basis |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 1024 | table_public_input | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 1024 | fixed_basis |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 65536 | table_public_input | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_phi8 | 65536 | fixed_basis |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 16 | production | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 16 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 16 | pfold |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 1024 | production | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 1024 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 1024 | pfold |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 65536 | production | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 65536 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_b127_mul | 65536 | pfold |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | production_blocked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | reuse_products | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | indexed_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | collect_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | vector_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | prepared_products |  | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_ood_reuse | log10 | allocate |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log10 | scratch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log10 | indexed_scratch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log16 | allocate | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log16 | scratch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log16 | indexed_scratch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log18 | allocate | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log18 | scratch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood_reuse | log18 | indexed_scratch |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | half_depth | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | half_depth | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | half_depth | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes32 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes32 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes32 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes32 | half_depth | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log16_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 23745.000 / 26295.990 | 96.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log16_lanes32 | depth_first |  | 0.942 [0.926, 0.956] | 0.973 [0.939, 1.033] | 22485.062 / 25232.932 | 96.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log16_lanes32 | tiled_half |  | 0.877 [0.867, 0.888] | 0.923 [0.886, 0.971] | 20931.666 / 23358.754 | 96.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log16_lanes32 | half_depth | yes | 0.877 [0.867, 0.886] | 0.895 [0.876, 0.960] | 20937.979 / 23745.746 | 96.000 | 0 / 0 | pass |
| aarch64 | opt_ntt | log18_lanes32 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log18_lanes32 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log18_lanes32 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log18_lanes32 | half_depth | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | production | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | half_depth |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs9_9 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs9_9 | single_write |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs9_9 | tiled_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs16_14 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs16_14 | single_write |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs16_14 | tiled_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs18_18 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs18_18 | single_write |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs18_18 | tiled_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs9_9 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs9_9 | single_write_ood |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs9_9 | tiled_indexed_ood | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs16_14 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs16_14 | single_write_ood |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs16_14 | tiled_indexed_ood | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs18_18 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs18_18 | single_write_ood |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs18_18 | tiled_indexed_ood | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_two_pair | 16 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_two_pair | 16 | separate_wide | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_two_pair | 1024 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_two_pair | 1024 | separate_wide | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fold_round | 16 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fold_round | 16 | fold_then_wide | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fold_round | 1024 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_fold_round | 1024 | fold_then_wide | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 16 | production_fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 16 | chunked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 1024 | production_fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 1024 | chunked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 16 | flock_scalar |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 16 | vector_chain | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 1024 | flock_scalar |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 1024 | vector_chain | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 65536 | flock_scalar |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf8_inverse | 65536 | vector_chain | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n16 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_zero_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_dense_prefix_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_sparse_prefix_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a1_b1_alternating_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | production_zero_skip | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n16 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_zero_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n16 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_dense_prefix_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n16 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_sparse_prefix_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n16 | row_density | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a9_b9_alternating_n1024 | row_density | yes | — | — | — | — | — | unmeasured |
