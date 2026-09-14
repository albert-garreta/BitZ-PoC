# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | two_limb_mac | signed16_n1 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1 | legacy_fused |  | 1.006 [0.998, 1.022] | 1.012 [0.981, 1.036] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n1 | product1 |  | 0.975 [0.967, 0.981] | 0.965 [0.929, 0.983] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1 | product2 |  | 0.972 [0.964, 0.978] | 0.965 [0.948, 0.986] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1 | product4 |  | 1.093 [1.087, 1.102] | 1.092 [1.069, 1.113] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1 | split1 |  | 1.004 [0.997, 1.010] | 0.998 [0.981, 1.026] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n1 | split2 |  | 1.094 [1.087, 1.099] | 1.091 [1.067, 1.106] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1 | split4 |  | 1.263 [1.258, 1.284] | 1.257 [1.237, 1.283] | 0.003 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1 | selected | yes | 1.090 [1.086, 1.099] | 1.083 [1.057, 1.098] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | legacy_fused |  | 1.087 [1.079, 1.092] | 1.083 [1.072, 1.094] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | product1 |  | 1.086 [1.079, 1.091] | 1.080 [1.071, 1.091] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | product2 |  | 1.003 [0.997, 1.010] | 0.994 [0.986, 1.004] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n3 | product4 |  | 0.982 [0.978, 0.988] | 0.982 [0.974, 0.992] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | split1 |  | 1.084 [1.079, 1.090] | 1.086 [1.073, 1.096] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | split2 |  | 1.144 [1.139, 1.150] | 1.145 [1.135, 1.155] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | split4 |  | 1.102 [1.095, 1.112] | 1.097 [1.083, 1.115] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | selected | yes | 1.002 [0.996, 1.007] | 1.001 [0.988, 1.007] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.007 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | legacy_fused |  | 1.097 [1.081, 1.114] | 1.095 [1.079, 1.113] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n7 | product1 |  | 0.908 [0.903, 0.912] | 0.907 [0.899, 0.920] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | product2 |  | 0.876 [0.868, 0.894] | 0.878 [0.867, 0.897] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | product4 |  | 0.925 [0.920, 0.937] | 0.936 [0.924, 0.943] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | split1 |  | 0.917 [0.911, 0.930] | 0.923 [0.912, 0.934] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | split2 |  | 0.949 [0.939, 0.959] | 0.953 [0.938, 0.964] | 0.006 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | split4 |  | 1.099 [1.083, 1.111] | 1.092 [1.075, 1.111] | 0.007 / 0.007 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n7 | selected | yes | 0.877 [0.870, 0.887] | 0.873 [0.867, 0.891] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | legacy_fused |  | 1.270 [1.264, 1.297] | 1.263 [1.248, 1.294] | 0.016 / 0.017 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n16 | product1 |  | 0.923 [0.921, 0.940] | 0.925 [0.911, 0.949] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | product2 |  | 0.878 [0.859, 0.902] | 0.885 [0.861, 0.900] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | product4 |  | 0.939 [0.931, 0.950] | 0.934 [0.924, 0.954] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | split1 |  | 0.918 [0.904, 0.931] | 0.924 [0.901, 0.933] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | split2 |  | 0.939 [0.935, 0.955] | 0.940 [0.932, 0.960] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | split4 |  | 1.035 [1.024, 1.051] | 1.034 [1.024, 1.059] | 0.013 / 0.014 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n16 | selected | yes | 0.875 [0.869, 0.884] | 0.879 [0.868, 0.892] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.016 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | legacy_fused |  | 1.276 [1.255, 1.307] | 1.272 [1.255, 1.308] | 0.017 / 0.021 | 0.001 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n17 | product1 |  | 0.928 [0.788, 0.932] | 0.930 [0.789, 0.937] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | product2 |  | 0.885 [0.855, 0.915] | 0.890 [0.869, 0.923] | 0.012 / 0.015 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | product4 |  | 0.937 [0.799, 0.949] | 0.936 [0.811, 0.957] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | split1 |  | 0.925 [0.795, 0.930] | 0.929 [0.798, 0.939] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | split2 |  | 0.940 [0.810, 0.947] | 0.944 [0.812, 0.958] | 0.013 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | split4 |  | 1.035 [0.896, 1.051] | 1.045 [0.896, 1.054] | 0.014 / 0.014 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n17 | selected | yes | 0.865 [0.741, 0.881] | 0.875 [0.749, 0.886] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.687 / 0.705 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | legacy_fused |  | 2.337 [2.329, 2.347] | 2.340 [2.319, 2.388] | 1.605 / 1.647 | 0.031 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1024 | product1 |  | 0.956 [0.952, 0.960] | 0.957 [0.946, 0.964] | 0.656 / 0.672 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | product2 |  | 0.953 [0.947, 0.957] | 0.956 [0.948, 0.967] | 0.655 / 0.670 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | product4 |  | 0.993 [0.986, 0.999] | 0.993 [0.984, 1.000] | 0.681 / 0.696 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | split1 |  | 0.955 [0.951, 0.959] | 0.953 [0.942, 0.964] | 0.656 / 0.669 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | split2 |  | 0.950 [0.947, 0.956] | 0.953 [0.945, 0.964] | 0.653 / 0.671 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | split4 |  | 0.981 [0.979, 0.989] | 0.982 [0.974, 0.994] | 0.675 / 0.690 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | selected | yes | 0.952 [0.949, 0.957] | 0.951 [0.941, 0.962] | 0.654 / 0.669 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 43.837 / 44.915 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | legacy_fused |  | 2.368 [2.357, 2.381] | 2.359 [2.327, 2.369] | 103.764 / 105.841 | 2.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n65536 | product1 |  | 0.950 [0.946, 0.957] | 0.953 [0.944, 0.961] | 41.696 / 42.791 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | product2 |  | 0.950 [0.945, 0.958] | 0.949 [0.941, 0.958] | 41.709 / 42.688 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | product4 |  | 0.993 [0.988, 0.997] | 0.991 [0.985, 0.998] | 43.529 / 44.669 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | split1 |  | 0.953 [0.947, 0.957] | 0.952 [0.945, 0.963] | 41.750 / 42.901 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | split2 |  | 0.952 [0.947, 0.958] | 0.953 [0.945, 0.957] | 41.788 / 42.914 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | split4 |  | 0.983 [0.979, 0.991] | 0.982 [0.975, 0.992] | 43.097 / 44.143 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | selected | yes | 0.950 [0.946, 0.957] | 0.953 [0.945, 0.961] | 41.689 / 42.783 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 722.750 / 756.501 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | legacy_fused |  | 2.304 [2.280, 2.313] | 2.284 [2.195, 2.302] | 1664.398 / 1681.802 | 32.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1048576 | product1 |  | 0.948 [0.946, 0.955] | 0.953 [0.938, 0.985] | 684.945 / 728.050 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | product2 |  | 0.946 [0.942, 0.953] | 0.958 [0.939, 0.987] | 682.578 / 731.172 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | product4 |  | 0.982 [0.979, 0.984] | 0.984 [0.975, 0.988] | 710.490 / 742.212 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | split1 |  | 0.948 [0.945, 0.957] | 0.954 [0.941, 0.969] | 685.185 / 725.298 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | split2 |  | 0.947 [0.944, 0.954] | 0.948 [0.938, 0.969] | 684.128 / 731.113 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | split4 |  | 0.973 [0.971, 0.978] | 0.974 [0.963, 0.999] | 702.823 / 743.132 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | selected | yes | 0.944 [0.940, 0.947] | 0.956 [0.935, 0.967] | 682.531 / 728.292 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | legacy_fused |  | 1.000 [0.996, 1.008] | 1.002 [0.994, 1.026] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n1 | product1 |  | 0.970 [0.966, 0.974] | 0.969 [0.962, 0.993] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | product2 |  | 0.970 [0.966, 0.972] | 0.970 [0.960, 0.998] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | product4 |  | 1.089 [1.084, 1.093] | 1.092 [1.085, 1.107] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1 | split1 |  | 0.999 [0.995, 1.003] | 1.003 [0.992, 1.019] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n1 | split2 |  | 1.089 [1.083, 1.096] | 1.095 [1.080, 1.104] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1 | split4 |  | 1.260 [1.255, 1.279] | 1.259 [1.250, 1.279] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1 | selected | yes | 1.089 [1.085, 1.093] | 1.092 [1.083, 1.105] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | legacy_fused |  | 1.080 [1.073, 1.088] | 1.084 [1.070, 1.095] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | product1 |  | 1.081 [1.072, 1.085] | 1.076 [1.068, 1.088] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | product2 |  | 0.996 [0.992, 1.003] | 0.998 [0.991, 1.009] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | product4 |  | 0.980 [0.973, 0.985] | 0.978 [0.970, 0.986] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | split1 |  | 1.081 [1.075, 1.089] | 1.083 [1.076, 1.095] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | split2 |  | 1.143 [1.136, 1.149] | 1.141 [1.134, 1.147] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | split4 |  | 1.103 [1.095, 1.110] | 1.101 [1.092, 1.109] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | selected | yes | 1.000 [0.993, 1.008] | 0.998 [0.989, 1.002] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.007 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | legacy_fused |  | 1.120 [1.105, 1.132] | 1.118 [1.104, 1.140] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n7 | product1 |  | 0.910 [0.902, 0.919] | 0.911 [0.897, 0.931] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | product2 |  | 0.881 [0.871, 0.896] | 0.880 [0.864, 0.899] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | product4 |  | 0.929 [0.921, 0.937] | 0.931 [0.907, 0.944] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | split1 |  | 0.913 [0.910, 0.923] | 0.918 [0.905, 0.927] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | split2 |  | 0.954 [0.942, 0.959] | 0.963 [0.932, 0.975] | 0.006 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | split4 |  | 1.100 [1.083, 1.109] | 1.099 [1.068, 1.116] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n7 | selected | yes | 0.880 [0.871, 0.890] | 0.876 [0.862, 0.896] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | legacy_fused |  | 1.303 [1.297, 1.314] | 1.309 [1.287, 1.325] | 0.017 / 0.017 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n16 | product1 |  | 0.931 [0.925, 0.936] | 0.935 [0.927, 0.946] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | product2 |  | 0.886 [0.864, 0.891] | 0.889 [0.868, 0.900] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | product4 |  | 0.937 [0.934, 0.951] | 0.943 [0.931, 0.951] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | split1 |  | 0.920 [0.908, 0.928] | 0.925 [0.911, 0.942] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | split2 |  | 0.943 [0.938, 0.952] | 0.944 [0.937, 0.958] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | split4 |  | 1.038 [1.031, 1.046] | 1.039 [1.033, 1.050] | 0.013 / 0.014 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n16 | selected | yes | 0.877 [0.871, 0.881] | 0.881 [0.871, 0.891] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.016 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | legacy_fused |  | 1.318 [1.300, 1.343] | 1.307 [1.141, 1.341] | 0.018 / 0.021 | 0.001 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n17 | product1 |  | 0.922 [0.786, 0.932] | 0.928 [0.782, 1.014] | 0.012 / 0.013 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n17 | product2 |  | 0.895 [0.870, 0.920] | 0.895 [0.872, 0.924] | 0.012 / 0.015 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | product4 |  | 0.934 [0.805, 0.947] | 0.934 [0.799, 0.946] | 0.013 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | split1 |  | 0.925 [0.800, 0.937] | 0.919 [0.800, 1.031] | 0.012 / 0.013 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n17 | split2 |  | 0.940 [0.808, 0.948] | 0.947 [0.802, 1.385] | 0.013 / 0.013 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n17 | split4 |  | 1.038 [0.889, 1.052] | 1.038 [0.689, 1.051] | 0.014 / 0.014 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n17 | selected | yes | 0.866 [0.734, 0.882] | 0.870 [0.729, 0.895] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.686 / 0.702 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | legacy_fused |  | 2.336 [2.328, 2.345] | 2.337 [2.321, 2.349] | 1.602 / 1.631 | 0.031 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1024 | product1 |  | 0.956 [0.952, 0.960] | 0.957 [0.943, 0.962] | 0.655 / 0.673 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | product2 |  | 0.952 [0.947, 0.957] | 0.953 [0.948, 0.959] | 0.653 / 0.669 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | product4 |  | 0.994 [0.990, 0.999] | 0.994 [0.988, 1.001] | 0.681 / 0.698 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | split1 |  | 0.951 [0.948, 0.957] | 0.951 [0.946, 0.959] | 0.653 / 0.670 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | split2 |  | 0.952 [0.947, 0.958] | 0.952 [0.946, 0.958] | 0.653 / 0.669 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | split4 |  | 0.983 [0.978, 0.986] | 0.983 [0.971, 0.997] | 0.674 / 0.691 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | selected | yes | 0.953 [0.949, 0.959] | 0.953 [0.950, 0.958] | 0.654 / 0.669 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 43.961 / 44.638 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | legacy_fused |  | 2.362 [2.350, 2.374] | 2.360 [2.341, 2.369] | 103.827 / 105.212 | 2.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n65536 | product1 |  | 0.951 [0.948, 0.956] | 0.952 [0.945, 0.954] | 41.864 / 42.403 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | product2 |  | 0.949 [0.946, 0.953] | 0.951 [0.944, 0.955] | 41.731 / 42.330 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | product4 |  | 0.993 [0.987, 0.996] | 0.996 [0.988, 1.006] | 43.573 / 44.444 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | split1 |  | 0.949 [0.946, 0.954] | 0.952 [0.947, 0.971] | 41.747 / 42.439 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | split2 |  | 0.952 [0.946, 0.956] | 0.954 [0.947, 0.962] | 41.815 / 42.553 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | split4 |  | 0.983 [0.980, 0.986] | 0.984 [0.975, 0.987] | 43.204 / 43.752 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | selected | yes | 0.950 [0.946, 0.953] | 0.952 [0.945, 0.956] | 41.720 / 42.462 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 721.557 / 745.367 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | legacy_fused |  | 2.306 [2.298, 2.310] | 2.290 [2.248, 2.308] | 1662.125 / 1698.216 | 32.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1048576 | product1 |  | 0.948 [0.945, 0.949] | 0.946 [0.942, 0.986] | 683.211 / 710.091 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | product2 |  | 0.943 [0.941, 0.945] | 0.942 [0.936, 0.954] | 680.617 / 713.855 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | product4 |  | 0.984 [0.981, 0.990] | 0.984 [0.973, 1.011] | 709.354 / 748.306 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n1048576 | split1 |  | 0.948 [0.947, 0.952] | 0.948 [0.942, 0.963] | 683.768 / 713.021 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | split2 |  | 0.946 [0.944, 0.948] | 0.944 [0.932, 0.955] | 682.831 / 707.983 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | split4 |  | 0.974 [0.971, 0.975] | 0.973 [0.964, 0.982] | 702.414 / 718.702 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | selected | yes | 0.944 [0.942, 0.946] | 0.942 [0.938, 0.971] | 681.010 / 713.439 | 32.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.330 / 0.338 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | fixed9 |  | 3.057 [3.005, 3.101] | 3.132 [3.083, 3.181] | 1.012 / 1.061 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active4_n16 | prepared_bound |  | 0.316 [0.313, 0.318] | 0.323 [0.317, 0.328] | 0.104 / 0.110 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute |  | 0.370 [0.365, 0.372] | 0.381 [0.369, 0.384] | 0.122 / 0.128 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | prepared_direct |  | 0.321 [0.313, 0.323] | 0.333 [0.313, 0.335] | 0.105 / 0.112 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute_direct |  | 0.374 [0.367, 0.376] | 0.380 [0.373, 0.385] | 0.123 / 0.129 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | public_bound4 |  | 0.317 [0.314, 0.319] | 0.330 [0.318, 0.332] | 0.105 / 0.111 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | prepared_comba4 |  | 0.324 [0.317, 0.329] | 0.335 [0.321, 0.342] | 0.107 / 0.114 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute_comba4 |  | 0.372 [0.366, 0.377] | 0.380 [0.370, 0.385] | 0.123 / 0.129 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | prepared_selected | yes | 0.317 [0.313, 0.318] | 0.327 [0.314, 0.329] | 0.104 / 0.110 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute_selected | yes | 0.372 [0.367, 0.374] | 0.377 [0.370, 0.383] | 0.122 / 0.129 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.361 / 21.934 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | fixed9 |  | 3.063 [2.980, 3.107] | 3.111 [3.055, 3.130] | 65.641 / 68.010 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active4_n1024 | prepared_bound |  | 0.314 [0.312, 0.318] | 0.323 [0.317, 0.327] | 6.747 / 7.120 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute |  | 0.401 [0.398, 0.403] | 0.407 [0.402, 0.415] | 8.585 / 8.988 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | prepared_direct |  | 0.318 [0.317, 0.320] | 0.328 [0.320, 0.335] | 6.818 / 7.271 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute_direct |  | 0.405 [0.403, 0.408] | 0.412 [0.406, 0.417] | 8.682 / 9.034 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | public_bound4 |  | 0.313 [0.311, 0.315] | 0.320 [0.314, 0.326] | 6.702 / 7.096 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | prepared_comba4 |  | 0.322 [0.319, 0.325] | 0.330 [0.325, 0.333] | 6.902 / 7.261 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute_comba4 |  | 0.407 [0.405, 0.411] | 0.415 [0.410, 0.420] | 8.730 / 9.163 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | prepared_selected | yes | 0.315 [0.313, 0.317] | 0.323 [0.317, 0.328] | 6.762 / 7.102 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute_selected | yes | 0.401 [0.398, 0.406] | 0.410 [0.400, 0.414] | 8.599 / 8.959 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1396.078 / 1438.960 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | fixed9 |  | 3.135 [3.111, 3.147] | 3.156 [3.129, 3.165] | 4373.630 / 4473.092 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active4_n65536 | prepared_bound |  | 0.327 [0.325, 0.328] | 0.327 [0.326, 0.334] | 456.464 / 465.661 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute |  | 0.433 [0.432, 0.434] | 0.435 [0.433, 0.444] | 605.292 / 616.581 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | prepared_direct |  | 0.334 [0.331, 0.335] | 0.336 [0.332, 0.347] | 465.453 / 481.024 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute_direct |  | 0.439 [0.438, 0.442] | 0.443 [0.441, 0.486] | 613.844 / 660.808 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | public_bound4 |  | 0.327 [0.323, 0.328] | 0.328 [0.326, 0.329] | 455.964 / 461.450 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | prepared_comba4 |  | 0.335 [0.332, 0.336] | 0.340 [0.335, 0.368] | 467.016 / 504.807 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute_comba4 |  | 0.441 [0.440, 0.443] | 0.452 [0.445, 0.477] | 616.255 / 666.895 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | prepared_selected | yes | 0.328 [0.327, 0.329] | 0.330 [0.328, 0.340] | 458.276 / 469.037 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute_selected | yes | 0.435 [0.433, 0.436] | 0.436 [0.433, 0.452] | 607.234 / 634.703 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.937 / 0.974 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | fixed9 |  | 1.081 [1.051, 1.093] | 1.097 [1.064, 1.111] | 1.010 / 1.060 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n16 | prepared_bound |  | 1.107 [1.098, 1.113] | 1.102 [1.063, 1.123] | 1.035 / 1.068 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n16 | validate_execute |  | 1.117 [1.098, 1.125] | 1.116 [1.084, 1.127] | 1.046 / 1.079 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n16 | prepared_direct |  | 0.924 [0.918, 0.949] | 0.933 [0.903, 0.951] | 0.868 / 0.906 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | validate_execute_direct |  | 0.921 [0.916, 0.937] | 0.929 [0.911, 0.942] | 0.866 / 0.901 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | prepared_selected | yes | 0.923 [0.915, 0.951] | 0.937 [0.909, 0.951] | 0.869 / 0.913 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | validate_execute_selected | yes | 0.921 [0.913, 0.941] | 0.945 [0.905, 0.955] | 0.868 / 0.908 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 60.121 / 63.624 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | fixed9 |  | 1.108 [1.091, 1.116] | 1.105 [1.073, 1.120] | 66.599 / 69.295 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n1024 | prepared_bound |  | 1.120 [1.110, 1.131] | 1.106 [1.086, 1.128] | 67.507 / 69.627 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n1024 | validate_execute |  | 1.110 [1.101, 1.124] | 1.116 [1.087, 1.131] | 67.021 / 69.952 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n1024 | prepared_direct |  | 0.913 [0.906, 0.918] | 0.905 [0.886, 0.919] | 54.788 / 56.900 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | validate_execute_direct |  | 0.913 [0.905, 0.920] | 0.898 [0.886, 0.917] | 54.795 / 57.033 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | prepared_selected | yes | 0.909 [0.901, 0.914] | 0.895 [0.880, 0.908] | 54.488 / 56.444 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | validate_execute_selected | yes | 0.913 [0.907, 0.921] | 0.899 [0.880, 0.915] | 55.025 / 56.486 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3891.354 / 4091.179 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | fixed9 |  | 1.120 [1.112, 1.127] | 1.102 [1.083, 1.123] | 4356.021 / 4520.668 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n65536 | prepared_bound |  | 1.124 [1.118, 1.134] | 1.102 [1.093, 1.122] | 4383.125 / 4526.739 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n65536 | validate_execute |  | 1.126 [1.115, 1.134] | 1.097 [1.087, 1.121] | 4379.417 / 4496.379 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n65536 | prepared_direct |  | 0.913 [0.904, 0.921] | 0.895 [0.882, 0.910] | 3550.604 / 3663.740 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | validate_execute_direct |  | 0.911 [0.905, 0.919] | 0.893 [0.880, 0.911] | 3546.688 / 3666.456 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | prepared_selected | yes | 0.910 [0.901, 0.921] | 0.900 [0.882, 0.914] | 3549.750 / 3669.408 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | validate_execute_selected | yes | 0.911 [0.904, 0.917] | 0.900 [0.883, 0.912] | 3546.854 / 3666.227 | 18.000 | 0 / 0 | pass |
