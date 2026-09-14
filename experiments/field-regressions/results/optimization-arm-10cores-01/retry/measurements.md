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
| aarch64 | opt_integer_mac | l1_signed16_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_signed16_n1024 | incumbent | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_integer_mac | l1_full_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n1024 | incumbent | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | products2 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | products4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | deferred_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_integer_mac | l1_full_n65536 | incumbent | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_projection | q128_l2_n16 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l2_n16 | public_divisor |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l2_n16 | horner_prepared | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_projection | q128_l2_n16 | horner_one_shot | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_uint_add | l1_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_add | l1_n1024 | word_carry | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_uint_sub | l1_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l1_n1024 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n16 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l2_n1024 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l4_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l4_n16 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l4_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l4_n1024 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l9_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l9_n16 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l9_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_sub | l9_n1024 | word_carry | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n16 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n16 | word_option | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_add | l1_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_uint_checked_sub | l2_n1024 | vendor_option |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_uint_checked_sub | l2_n1024 | word_option | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_prime_dot | q100_n16 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n16 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n16 | length_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n1024 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n1024 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n1024 | length_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n65536 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n65536 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q100_n65536 | length_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n16 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n16 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n16 | length_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n1024 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n1024 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n1024 | length_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n65536 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n65536 | acc4 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_dot | q128_n65536 | length_dispatch | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n16 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n16 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n1024 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n65536 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q100_n65536 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n16 | one_acc |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_prime_linear | q128_n16 | acc4 | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_ood_reuse | log16 | allocate |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 101.956 / 113.532 | 1.000 | 2 / 65792 | pass |
| aarch64 | opt_ood_reuse | log16 | scratch | yes | 0.997 [0.975, 1.016] | 1.014 [0.983, 1.041] | 101.249 / 114.727 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ood_reuse | log18 | allocate |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 140.404 / 205.547 | 4.000 | 2 / 66560 | pass |
| aarch64 | opt_ood_reuse | log18 | scratch | yes | 1.016 [0.988, 1.027] | 0.936 [0.660, 1.020] | 142.206 / 161.179 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log8_lanes32 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | depth_first | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | depth_first | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes8 | depth_first | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log15_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2949.959 / 4262.944 | 48.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log15_lanes32 | depth_first | yes | 0.988 [0.932, 1.045] | 1.000 [0.850, 1.205] | 2907.802 / 3902.117 | 48.000 | 1 / 1520 | regression |
| aarch64 | opt_ntt | log16_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 5581.979 / 7185.856 | 96.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log16_lanes32 | depth_first | yes | 1.015 [0.973, 1.083] | 0.982 [0.795, 1.315] | 5730.105 / 7520.749 | 96.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log18_lanes32 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 23168.542 / 29522.058 | 384.000 | 1 / 1520 | pass |
| aarch64 | opt_ntt | log18_lanes32 | depth_first | yes | 1.006 [0.985, 1.021] | 0.865 [0.746, 0.953] | 23292.688 / 25053.577 | 384.000 | 0 / 0 | inconclusive |
| aarch64 | opt_ntt | log8_lanes1 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs9_9 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs9_9 | single_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs16_14 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs16_14 | single_write | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_pack | logs18_18 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 428.604 / 565.575 | 8.000 | 1 / 8388608 | pass |
| aarch64 | opt_pack | logs18_18 | single_write | yes | 1.054 [0.954, 1.073] | 0.938 [0.678, 0.985] | 448.172 / 476.219 | 8.000 | 1 / 8388608 | inconclusive |
| aarch64 | opt_packed_ood | logs9_9 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs9_9 | single_write_ood | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs16_14 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs16_14 | single_write_ood | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_packed_ood | logs18_18 | production |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 589.951 / 676.660 | 16.000 | 23 / 8525792 | pass |
| aarch64 | opt_packed_ood | logs18_18 | single_write_ood | yes | 1.181 [1.148, 1.190] | 1.064 [1.014, 1.115] | 689.823 / 728.882 | 16.000 | 3 / 8456192 | regression |
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
