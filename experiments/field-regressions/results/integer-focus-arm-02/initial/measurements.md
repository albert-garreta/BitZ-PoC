# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | two_limb_mac | signed16_n1 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1 | legacy_fused |  | 0.999 [0.990, 1.004] | 0.995 [0.977, 1.022] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n1 | product1 |  | 0.971 [0.959, 0.976] | 0.963 [0.951, 0.993] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1 | product2 |  | 0.972 [0.965, 0.988] | 0.980 [0.964, 0.995] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1 | product4 |  | 1.092 [1.083, 1.099] | 1.088 [1.069, 1.100] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1 | split1 |  | 0.998 [0.991, 1.004] | 1.017 [0.985, 1.085] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n1 | split2 |  | 1.096 [1.086, 1.102] | 1.094 [1.075, 1.157] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1 | split4 |  | 1.263 [1.255, 1.271] | 1.253 [1.237, 1.270] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1 | selected | yes | 0.964 [0.959, 0.970] | 0.958 [0.945, 0.978] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | legacy_fused |  | 1.003 [0.990, 1.014] | 1.008 [0.993, 1.018] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n3 | product1 |  | 1.083 [1.074, 1.091] | 1.083 [1.069, 1.088] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | product2 |  | 0.999 [0.993, 1.005] | 1.000 [0.984, 1.015] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n3 | product4 |  | 0.980 [0.971, 0.986] | 0.987 [0.971, 1.002] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | split1 |  | 1.084 [1.076, 1.090] | 1.086 [1.071, 1.098] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | split2 |  | 1.140 [1.130, 1.147] | 1.143 [1.128, 1.158] | 0.004 / 0.005 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | split4 |  | 1.109 [1.099, 1.113] | 1.106 [1.083, 1.119] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | selected | yes | 0.995 [0.990, 1.002] | 1.006 [0.990, 1.014] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n7 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.007 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | legacy_fused |  | 1.123 [1.117, 1.147] | 1.128 [1.109, 1.142] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n7 | product1 |  | 0.907 [0.898, 0.916] | 0.905 [0.896, 0.916] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | product2 |  | 0.880 [0.874, 0.902] | 0.885 [0.878, 0.901] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | product4 |  | 0.936 [0.929, 0.948] | 0.939 [0.929, 0.951] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | split1 |  | 0.923 [0.915, 0.928] | 0.923 [0.915, 0.935] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | split2 |  | 0.956 [0.948, 0.969] | 0.962 [0.947, 0.971] | 0.006 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | split4 |  | 1.108 [1.096, 1.119] | 1.104 [1.092, 1.117] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n7 | selected | yes | 0.884 [0.871, 0.893] | 0.886 [0.879, 0.903] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | legacy_fused |  | 1.298 [1.282, 1.326] | 1.287 [1.255, 1.313] | 0.016 / 0.017 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n16 | product1 |  | 0.933 [0.924, 0.947] | 0.940 [0.922, 0.947] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | product2 |  | 0.868 [0.860, 0.892] | 0.855 [0.840, 0.891] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | product4 |  | 0.943 [0.932, 0.948] | 0.936 [0.925, 0.952] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | split1 |  | 0.910 [0.906, 0.935] | 0.921 [0.900, 0.928] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | split2 |  | 0.949 [0.939, 0.961] | 0.943 [0.933, 0.962] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | split4 |  | 1.044 [1.024, 1.052] | 1.037 [1.011, 1.052] | 0.013 / 0.013 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n16 | selected | yes | 0.873 [0.863, 0.878] | 0.868 [0.854, 0.887] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | legacy_fused |  | 1.330 [1.295, 1.348] | 1.314 [1.291, 1.342] | 0.017 / 0.018 | 0.001 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n17 | product1 |  | 0.930 [0.917, 0.938] | 0.935 [0.920, 0.947] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | product2 |  | 0.899 [0.887, 0.905] | 0.896 [0.887, 0.906] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | product4 |  | 0.942 [0.933, 0.951] | 0.939 [0.931, 0.963] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | split1 |  | 0.928 [0.907, 0.943] | 0.923 [0.910, 0.948] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | split2 |  | 0.943 [0.933, 0.950] | 0.945 [0.925, 0.955] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | split4 |  | 1.046 [1.034, 1.056] | 1.042 [1.025, 1.062] | 0.014 / 0.014 | 0.001 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n17 | selected | yes | 0.880 [0.868, 0.889] | 0.884 [0.864, 0.895] | 0.011 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.681 / 0.694 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | legacy_fused |  | 2.345 [2.334, 2.353] | 2.322 [2.309, 2.332] | 1.596 / 1.612 | 0.031 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1024 | product1 |  | 0.951 [0.947, 0.958] | 0.953 [0.945, 0.960] | 0.649 / 0.660 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | product2 |  | 0.952 [0.946, 0.957] | 0.952 [0.948, 0.962] | 0.647 / 0.663 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | product4 |  | 0.992 [0.988, 0.997] | 0.994 [0.983, 0.999] | 0.676 / 0.689 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | split1 |  | 0.952 [0.948, 0.958] | 0.953 [0.943, 0.956] | 0.649 / 0.660 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | split2 |  | 0.953 [0.947, 0.957] | 0.953 [0.946, 0.957] | 0.648 / 0.661 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | split4 |  | 0.980 [0.976, 0.986] | 0.981 [0.974, 0.988] | 0.668 / 0.681 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | selected | yes | 0.949 [0.946, 0.956] | 0.954 [0.943, 0.959] | 0.648 / 0.661 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 43.443 / 44.661 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | legacy_fused |  | 2.363 [2.356, 2.377] | 2.362 [2.343, 2.434] | 102.784 / 105.227 | 2.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n65536 | product1 |  | 0.948 [0.943, 0.956] | 0.954 [0.944, 0.962] | 41.238 / 42.617 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | product2 |  | 0.947 [0.941, 0.953] | 0.954 [0.941, 0.963] | 41.138 / 42.368 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | product4 |  | 0.990 [0.984, 0.998] | 0.992 [0.976, 0.998] | 43.007 / 44.174 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | split1 |  | 0.946 [0.942, 0.958] | 0.950 [0.942, 0.972] | 41.215 / 42.494 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | split2 |  | 0.947 [0.943, 0.954] | 0.950 [0.941, 0.957] | 41.195 / 42.306 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | split4 |  | 0.984 [0.976, 0.990] | 0.982 [0.973, 0.990] | 42.715 / 43.883 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | selected | yes | 0.946 [0.941, 0.953] | 0.948 [0.941, 0.958] | 41.183 / 42.155 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 720.552 / 758.073 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | legacy_fused |  | 2.303 [2.270, 2.309] | 2.261 [2.208, 2.305] | 1661.023 / 1691.130 | 32.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1048576 | product1 |  | 0.947 [0.946, 0.954] | 0.950 [0.934, 0.972] | 682.758 / 728.159 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | product2 |  | 0.944 [0.943, 0.950] | 0.944 [0.818, 0.977] | 680.458 / 723.043 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | product4 |  | 0.982 [0.980, 0.985] | 0.988 [0.515, 1.004] | 707.958 / 746.070 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | split1 |  | 0.948 [0.946, 0.953] | 0.964 [0.739, 0.983] | 682.651 / 726.026 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | split2 |  | 0.946 [0.945, 0.952] | 0.944 [0.688, 0.975] | 682.083 / 717.379 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | split4 |  | 0.972 [0.968, 0.974] | 0.980 [0.710, 0.991] | 700.539 / 745.744 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | selected | yes | 0.944 [0.943, 0.952] | 0.945 [0.891, 0.979] | 680.865 / 730.138 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | legacy_fused |  | 1.002 [0.990, 1.009] | 1.007 [0.996, 1.028] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n1 | product1 |  | 0.970 [0.959, 0.978] | 0.967 [0.952, 0.984] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | product2 |  | 0.973 [0.964, 0.979] | 0.975 [0.959, 0.998] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | product4 |  | 1.090 [1.083, 1.101] | 1.093 [1.078, 1.115] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1 | split1 |  | 1.000 [0.992, 1.010] | 1.003 [0.991, 1.024] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n1 | split2 |  | 1.096 [1.086, 1.105] | 1.083 [1.071, 1.113] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1 | split4 |  | 1.270 [1.255, 1.275] | 1.258 [1.247, 1.285] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1 | selected | yes | 0.969 [0.960, 0.981] | 0.965 [0.953, 0.979] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | legacy_fused |  | 1.003 [0.997, 1.007] | 1.007 [0.994, 1.012] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n3 | product1 |  | 1.080 [1.074, 1.088] | 1.081 [1.068, 1.094] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | product2 |  | 0.995 [0.988, 1.005] | 1.001 [0.988, 1.009] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | product4 |  | 0.980 [0.973, 0.983] | 0.986 [0.974, 0.998] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | split1 |  | 1.081 [1.072, 1.088] | 1.084 [1.065, 1.092] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | split2 |  | 1.144 [1.132, 1.148] | 1.148 [1.130, 1.153] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | split4 |  | 1.096 [1.091, 1.106] | 1.107 [1.091, 1.117] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | selected | yes | 1.000 [0.993, 1.007] | 1.002 [0.990, 1.012] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n7 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.007 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | legacy_fused |  | 1.123 [1.119, 1.147] | 1.128 [1.113, 1.141] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n7 | product1 |  | 0.914 [0.905, 0.922] | 0.924 [0.910, 0.932] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | product2 |  | 0.883 [0.874, 0.891] | 0.898 [0.874, 0.914] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | product4 |  | 0.939 [0.933, 0.949] | 0.948 [0.929, 0.959] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | split1 |  | 0.922 [0.914, 0.930] | 0.927 [0.906, 0.944] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | split2 |  | 0.964 [0.957, 0.972] | 0.959 [0.950, 0.974] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | split4 |  | 1.110 [1.103, 1.124] | 1.119 [1.101, 1.133] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n7 | selected | yes | 0.885 [0.879, 0.891] | 0.887 [0.880, 0.907] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | legacy_fused |  | 1.308 [1.297, 1.317] | 1.298 [1.280, 1.322] | 0.016 / 0.017 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n16 | product1 |  | 0.933 [0.927, 0.939] | 0.935 [0.925, 0.950] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | product2 |  | 0.871 [0.864, 0.876] | 0.874 [0.865, 0.886] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | product4 |  | 0.942 [0.934, 0.946] | 0.944 [0.930, 0.953] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | split1 |  | 0.912 [0.905, 0.928] | 0.913 [0.902, 0.929] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | split2 |  | 0.949 [0.940, 0.954] | 0.948 [0.937, 0.970] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | split4 |  | 1.045 [1.027, 1.050] | 1.041 [1.018, 1.055] | 0.013 / 0.013 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n16 | selected | yes | 0.867 [0.860, 0.874] | 0.875 [0.855, 0.884] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | legacy_fused |  | 1.332 [1.321, 1.344] | 1.325 [1.308, 1.332] | 0.017 / 0.018 | 0.001 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n17 | product1 |  | 0.929 [0.918, 0.937] | 0.930 [0.921, 0.944] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | product2 |  | 0.898 [0.886, 0.904] | 0.899 [0.889, 0.909] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | product4 |  | 0.937 [0.924, 0.949] | 0.942 [0.930, 0.956] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | split1 |  | 0.921 [0.913, 0.925] | 0.927 [0.912, 0.937] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | split2 |  | 0.947 [0.927, 0.951] | 0.940 [0.919, 0.959] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | split4 |  | 1.042 [1.025, 1.051] | 1.045 [1.032, 1.060] | 0.014 / 0.014 | 0.001 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n17 | selected | yes | 0.881 [0.868, 0.885] | 0.885 [0.871, 0.890] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.682 / 0.693 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | legacy_fused |  | 2.340 [2.331, 2.346] | 2.327 [2.319, 2.354] | 1.596 / 1.616 | 0.031 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1024 | product1 |  | 0.953 [0.948, 0.958] | 0.958 [0.951, 0.969] | 0.649 / 0.664 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | product2 |  | 0.946 [0.944, 0.952] | 0.955 [0.945, 0.959] | 0.646 / 0.660 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | product4 |  | 0.990 [0.984, 0.994] | 0.992 [0.987, 0.997] | 0.675 / 0.688 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | split1 |  | 0.951 [0.947, 0.955] | 0.955 [0.950, 0.967] | 0.649 / 0.662 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | split2 |  | 0.950 [0.947, 0.957] | 0.956 [0.952, 0.964] | 0.649 / 0.663 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | split4 |  | 0.981 [0.975, 0.984] | 0.982 [0.979, 0.995] | 0.669 / 0.683 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | selected | yes | 0.949 [0.945, 0.954] | 0.958 [0.950, 0.967] | 0.647 / 0.660 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 43.358 / 44.575 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | legacy_fused |  | 2.373 [2.358, 2.387] | 2.352 [2.324, 2.376] | 102.936 / 104.431 | 2.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n65536 | product1 |  | 0.953 [0.942, 0.960] | 0.952 [0.937, 0.964] | 41.207 / 42.191 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | product2 |  | 0.950 [0.945, 0.953] | 0.944 [0.937, 0.960] | 41.160 / 42.012 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | product4 |  | 0.994 [0.988, 0.998] | 0.992 [0.982, 1.000] | 43.062 / 43.976 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | split1 |  | 0.954 [0.946, 0.957] | 0.946 [0.940, 0.960] | 41.271 / 42.363 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | split2 |  | 0.953 [0.946, 0.956] | 0.948 [0.938, 0.957] | 41.326 / 42.125 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | split4 |  | 0.983 [0.977, 0.989] | 0.979 [0.973, 1.001] | 42.639 / 43.723 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | selected | yes | 0.951 [0.943, 0.954] | 0.944 [0.937, 0.962] | 41.179 / 42.166 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 722.216 / 753.465 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | legacy_fused |  | 2.303 [2.283, 2.308] | 2.288 [2.251, 2.309] | 1662.995 / 1736.178 | 32.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1048576 | product1 |  | 0.948 [0.946, 0.964] | 0.955 [0.941, 0.982] | 684.794 / 727.400 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | product2 |  | 0.945 [0.940, 0.956] | 0.963 [0.943, 1.004] | 681.620 / 729.966 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | product4 |  | 0.983 [0.980, 0.986] | 0.984 [0.974, 1.003] | 709.984 / 746.787 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | split1 |  | 0.947 [0.944, 0.952] | 0.950 [0.942, 0.964] | 683.943 / 726.206 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | split2 |  | 0.946 [0.944, 0.959] | 0.952 [0.941, 0.976] | 683.466 / 722.712 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | split4 |  | 0.974 [0.972, 0.981] | 0.976 [0.968, 1.018] | 703.438 / 745.690 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n1048576 | selected | yes | 0.944 [0.942, 0.961] | 0.954 [0.939, 0.962] | 681.523 / 719.051 | 32.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.328 / 0.335 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | fixed9 |  | 3.087 [3.059, 3.119] | 3.169 [3.072, 3.182] | 1.015 / 1.059 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active4_n16 | prepared_bound |  | 0.314 [0.312, 0.317] | 0.325 [0.313, 0.330] | 0.103 / 0.109 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute |  | 0.370 [0.368, 0.375] | 0.376 [0.369, 0.387] | 0.122 / 0.127 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | prepared_direct |  | 0.320 [0.318, 0.322] | 0.333 [0.320, 0.336] | 0.105 / 0.112 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute_direct |  | 0.376 [0.373, 0.381] | 0.388 [0.377, 0.397] | 0.124 / 0.132 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | public_bound4 |  | 0.315 [0.314, 0.317] | 0.321 [0.313, 0.333] | 0.103 / 0.110 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | prepared_comba4 |  | 0.324 [0.323, 0.326] | 0.332 [0.329, 0.366] | 0.107 / 0.113 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute_comba4 |  | 0.378 [0.374, 0.381] | 0.385 [0.377, 0.389] | 0.124 / 0.130 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | prepared_selected | yes | 0.316 [0.314, 0.317] | 0.326 [0.317, 0.330] | 0.103 / 0.109 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute_selected | yes | 0.371 [0.368, 0.378] | 0.382 [0.370, 0.386] | 0.123 / 0.128 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.101 / 21.815 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | fixed9 |  | 3.123 [3.098, 3.156] | 3.166 [3.132, 3.184] | 65.959 / 68.481 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active4_n1024 | prepared_bound |  | 0.316 [0.314, 0.318] | 0.324 [0.318, 0.332] | 6.665 / 7.082 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute |  | 0.401 [0.399, 0.404] | 0.410 [0.404, 0.418] | 8.464 / 8.993 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | prepared_direct |  | 0.319 [0.317, 0.321] | 0.331 [0.325, 0.338] | 6.728 / 7.213 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute_direct |  | 0.408 [0.404, 0.410] | 0.421 [0.412, 0.425] | 8.591 / 9.061 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | public_bound4 |  | 0.314 [0.313, 0.316] | 0.328 [0.314, 0.335] | 6.627 / 7.115 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | prepared_comba4 |  | 0.323 [0.321, 0.325] | 0.336 [0.325, 0.345] | 6.809 / 7.292 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute_comba4 |  | 0.408 [0.405, 0.410] | 0.417 [0.409, 0.424] | 8.606 / 9.043 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | prepared_selected | yes | 0.317 [0.315, 0.319] | 0.328 [0.319, 0.332] | 6.670 / 7.103 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute_selected | yes | 0.402 [0.400, 0.407] | 0.416 [0.406, 0.418] | 8.493 / 8.961 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1391.901 / 1431.301 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | fixed9 |  | 3.127 [3.099, 3.149] | 3.107 [3.040, 3.168] | 4341.042 / 4449.053 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active4_n65536 | prepared_bound |  | 0.328 [0.326, 0.330] | 0.333 [0.327, 0.347] | 456.042 / 489.177 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute |  | 0.433 [0.430, 0.437] | 0.454 [0.430, 0.463] | 602.349 / 649.809 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | prepared_direct |  | 0.331 [0.326, 0.336] | 0.335 [0.330, 0.346] | 460.818 / 491.080 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute_direct |  | 0.436 [0.432, 0.440] | 0.458 [0.434, 0.465] | 605.693 / 658.476 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | public_bound4 |  | 0.327 [0.324, 0.332] | 0.335 [0.325, 0.346] | 455.073 / 487.398 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | prepared_comba4 |  | 0.334 [0.332, 0.336] | 0.342 [0.332, 0.352] | 463.964 / 497.976 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute_comba4 |  | 0.439 [0.435, 0.444] | 0.454 [0.422, 0.465] | 610.958 / 653.394 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | prepared_selected | yes | 0.328 [0.325, 0.330] | 0.342 [0.327, 0.346] | 456.089 / 488.210 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute_selected | yes | 0.437 [0.434, 0.440] | 0.452 [0.434, 0.459] | 605.406 / 649.366 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.933 / 0.967 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | fixed9 |  | 1.085 [1.060, 1.103] | 1.094 [1.086, 1.120] | 1.010 / 1.069 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n16 | prepared_bound |  | 1.096 [1.087, 1.111] | 1.097 [1.085, 1.108] | 1.026 / 1.066 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n16 | validate_execute |  | 1.097 [1.074, 1.118] | 1.104 [1.091, 1.126] | 1.023 / 1.073 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n16 | prepared_direct |  | 0.945 [0.935, 0.952] | 0.945 [0.932, 0.958] | 0.881 / 0.918 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | validate_execute_direct |  | 0.936 [0.925, 0.942] | 0.935 [0.922, 0.951] | 0.872 / 0.911 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | prepared_selected | yes | 0.939 [0.932, 0.951] | 0.942 [0.930, 0.952] | 0.880 / 0.916 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | validate_execute_selected | yes | 0.932 [0.924, 0.942] | 0.937 [0.917, 0.949] | 0.872 / 0.905 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 59.635 / 63.732 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | fixed9 |  | 1.107 [1.089, 1.133] | 1.104 [1.069, 1.129] | 66.403 / 70.714 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n1024 | prepared_bound |  | 1.122 [1.111, 1.134] | 1.104 [1.069, 1.132] | 67.091 / 71.269 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n1024 | validate_execute |  | 1.128 [1.102, 1.134] | 1.104 [1.064, 1.134] | 66.682 / 71.108 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n1024 | prepared_direct |  | 0.920 [0.913, 0.929] | 0.904 [0.869, 0.923] | 54.832 / 57.434 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | validate_execute_direct |  | 0.920 [0.909, 0.925] | 0.897 [0.872, 0.921] | 54.726 / 57.572 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | prepared_selected | yes | 0.918 [0.909, 0.924] | 0.893 [0.866, 0.924] | 54.561 / 57.600 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | validate_execute_selected | yes | 0.916 [0.909, 0.922] | 0.909 [0.867, 0.925] | 54.598 / 57.643 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3898.979 / 4210.911 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | fixed9 |  | 1.121 [1.109, 1.129] | 1.096 [1.055, 1.117] | 4365.000 / 4558.208 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n65536 | prepared_bound |  | 1.116 [1.106, 1.128] | 1.087 [0.962, 1.110] | 4358.146 / 4495.471 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n65536 | validate_execute |  | 1.113 [1.102, 1.122] | 1.092 [0.993, 1.112] | 4350.188 / 4557.600 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n65536 | prepared_direct |  | 0.907 [0.899, 0.915] | 0.900 [0.872, 0.915] | 3540.666 / 3669.247 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | validate_execute_direct |  | 0.908 [0.894, 0.916] | 0.885 [0.852, 0.904] | 3524.479 / 3655.602 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | prepared_selected | yes | 0.903 [0.894, 0.913] | 0.887 [0.867, 0.909] | 3523.062 / 3716.471 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | validate_execute_selected | yes | 0.908 [0.899, 0.919] | 0.890 [0.869, 0.920] | 3544.584 / 3685.015 | 18.000 | 0 / 0 | pass |
