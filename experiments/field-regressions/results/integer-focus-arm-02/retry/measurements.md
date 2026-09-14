# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | two_limb_mac | signed16_n1 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n3 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | legacy_fused |  | 1.002 [0.999, 1.010] | 0.997 [0.990, 1.012] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n3 | product1 |  | 1.082 [1.078, 1.088] | 1.077 [1.057, 1.091] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | product2 |  | 1.000 [0.996, 1.005] | 0.994 [0.980, 1.009] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | product4 |  | 0.980 [0.976, 0.986] | 0.987 [0.972, 0.997] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | split1 |  | 1.082 [1.079, 1.086] | 1.077 [1.063, 1.088] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | split2 |  | 1.143 [1.137, 1.147] | 1.134 [1.119, 1.149] | 0.004 / 0.005 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | split4 |  | 1.105 [1.098, 1.111] | 1.096 [1.089, 1.119] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | selected | yes | 1.003 [0.999, 1.006] | 0.992 [0.983, 1.007] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n7 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n7 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n7 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n7 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n7 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n7 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n7 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n7 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n16 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n16 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n16 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n16 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n16 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n16 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n16 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n16 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n17 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n17 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n17 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n17 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n17 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n17 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n17 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n17 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n17 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1024 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1024 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1024 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1024 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1024 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1024 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1024 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1024 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n65536 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n65536 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n65536 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n65536 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n65536 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n65536 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n65536 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n65536 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1048576 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1048576 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1048576 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1048576 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1048576 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1048576 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1048576 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1048576 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | signed16_n1048576 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n3 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | legacy_fused |  | 1.003 [0.997, 1.015] | 1.010 [0.994, 1.016] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n3 | product1 |  | 1.084 [1.080, 1.089] | 1.079 [1.073, 1.090] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | product2 |  | 1.001 [0.991, 1.006] | 1.002 [0.991, 1.011] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n3 | product4 |  | 0.980 [0.977, 0.987] | 0.986 [0.975, 0.991] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | split1 |  | 1.083 [1.079, 1.090] | 1.081 [1.073, 1.091] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | split2 |  | 1.141 [1.135, 1.148] | 1.141 [1.131, 1.150] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | split4 |  | 1.108 [1.100, 1.112] | 1.107 [1.092, 1.116] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | selected | yes | 1.000 [0.995, 1.007] | 1.001 [0.994, 1.010] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n7 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n7 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n7 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n7 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n7 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n7 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n7 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n7 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n7 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n16 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n16 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n16 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n16 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n16 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n16 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n16 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n16 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n17 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n17 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n17 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n17 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n17 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n17 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n17 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n17 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n17 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1024 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1024 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1024 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1024 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1024 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1024 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1024 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1024 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n65536 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n65536 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n65536 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n65536 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n65536 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n65536 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n65536 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n65536 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1048576 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1048576 | legacy_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1048576 | product1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1048576 | product2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1048576 | product4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1048576 | split1 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1048576 | split2 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1048576 | split4 |  | — | — | — | — | — | unmeasured |
| aarch64 | two_limb_mac | full128_n1048576 | selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | prepared_bound |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | validate_execute |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | prepared_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | validate_execute_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | public_bound4 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | prepared_comba4 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | validate_execute_comba4 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | prepared_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n16 | validate_execute_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | prepared_bound |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | validate_execute |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | prepared_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | validate_execute_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | public_bound4 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | prepared_comba4 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | validate_execute_comba4 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | prepared_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n1024 | validate_execute_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | prepared_bound |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | validate_execute |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | prepared_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | validate_execute_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | public_bound4 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | prepared_comba4 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | validate_execute_comba4 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | prepared_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active4_n65536 | validate_execute_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n16 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n16 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n16 | prepared_bound |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n16 | validate_execute |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n16 | prepared_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n16 | validate_execute_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n16 | prepared_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n16 | validate_execute_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n1024 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n1024 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n1024 | prepared_bound |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n1024 | validate_execute |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n1024 | prepared_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n1024 | validate_execute_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n1024 | prepared_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n1024 | validate_execute_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n65536 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n65536 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n65536 | prepared_bound |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n65536 | validate_execute |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n65536 | prepared_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n65536 | validate_execute_direct |  | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n65536 | prepared_selected | yes | — | — | — | — | — | unmeasured |
| aarch64 | bounded_product | active9_n65536 | validate_execute_selected | yes | — | — | — | — | — | unmeasured |
