# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | two_limb_mac | signed16_n1 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1 | legacy_fused |  | 1.002 [0.999, 1.008] | 1.005 [0.996, 1.010] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n1 | product1 |  | 0.971 [0.967, 0.977] | 0.969 [0.965, 0.982] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1 | product2 |  | 0.972 [0.967, 0.976] | 0.972 [0.965, 0.982] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1 | product4 |  | 1.098 [1.091, 1.103] | 1.093 [1.088, 1.110] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1 | seeded2 |  | 0.873 [0.869, 0.896] | 0.874 [0.867, 0.901] | 0.002 / 0.002 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1 | split1 |  | 1.003 [0.997, 1.010] | 0.999 [0.995, 1.018] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n1 | split2 |  | 1.096 [1.088, 1.103] | 1.090 [1.084, 1.117] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1 | split4 |  | 1.265 [1.261, 1.273] | 1.260 [1.253, 1.280] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1 | selected | yes | 0.886 [0.878, 0.911] | 0.900 [0.877, 0.915] | 0.002 / 0.002 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | legacy_fused |  | 1.083 [1.078, 1.091] | 1.077 [1.063, 1.087] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | product1 |  | 1.005 [0.996, 1.009] | 1.005 [0.987, 1.017] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n3 | product2 |  | 1.004 [0.996, 1.008] | 0.995 [0.986, 1.005] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | product4 |  | 0.983 [0.978, 0.987] | 0.981 [0.972, 0.992] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | seeded2 |  | 0.868 [0.862, 0.872] | 0.866 [0.857, 0.879] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | split1 |  | 1.001 [0.997, 1.008] | 0.999 [0.984, 1.005] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n3 | split2 |  | 1.143 [1.136, 1.150] | 1.141 [1.120, 1.149] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | split4 |  | 1.101 [1.095, 1.109] | 1.099 [1.070, 1.108] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n3 | selected | yes | 0.868 [0.864, 0.875] | 0.873 [0.851, 0.880] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.007 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | legacy_fused |  | 1.110 [1.084, 1.134] | 1.108 [1.079, 1.129] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n7 | product1 |  | 0.909 [0.896, 0.912] | 0.909 [0.899, 0.919] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | product2 |  | 0.880 [0.869, 0.885] | 0.878 [0.866, 0.890] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | product4 |  | 0.932 [0.926, 0.938] | 0.938 [0.921, 0.944] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | seeded2 |  | 0.825 [0.818, 0.829] | 0.827 [0.814, 0.838] | 0.005 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | split1 |  | 0.912 [0.906, 0.920] | 0.918 [0.902, 0.926] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | split2 |  | 0.960 [0.953, 0.972] | 0.959 [0.954, 0.984] | 0.006 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n7 | split4 |  | 1.102 [1.092, 1.111] | 1.105 [1.085, 1.112] | 0.007 / 0.007 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n7 | selected | yes | 0.818 [0.814, 0.829] | 0.825 [0.816, 0.833] | 0.005 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | legacy_fused |  | 1.282 [1.263, 1.325] | 1.275 [1.260, 1.323] | 0.016 / 0.017 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n16 | product1 |  | 0.928 [0.923, 0.937] | 0.931 [0.925, 0.937] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | product2 |  | 0.877 [0.868, 0.883] | 0.876 [0.867, 0.889] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | product4 |  | 0.945 [0.940, 0.954] | 0.952 [0.939, 0.959] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | seeded2 |  | 0.872 [0.856, 0.894] | 0.876 [0.854, 0.898] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | split1 |  | 0.919 [0.912, 0.924] | 0.923 [0.914, 0.934] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | split2 |  | 0.959 [0.950, 0.974] | 0.960 [0.955, 0.978] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n16 | split4 |  | 1.047 [1.042, 1.054] | 1.045 [1.038, 1.054] | 0.013 / 0.013 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n16 | selected | yes | 0.863 [0.852, 0.885] | 0.866 [0.858, 0.885] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | legacy_fused |  | 1.297 [1.278, 1.337] | 1.310 [1.265, 1.329] | 0.017 / 0.018 | 0.001 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n17 | product1 |  | 0.914 [0.909, 0.924] | 0.914 [0.906, 0.935] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | product2 |  | 0.881 [0.875, 0.890] | 0.886 [0.874, 0.894] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | product4 |  | 0.952 [0.939, 0.959] | 0.954 [0.936, 0.965] | 0.013 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | seeded2 |  | 0.870 [0.863, 0.882] | 0.880 [0.862, 0.892] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | split1 |  | 0.911 [0.906, 0.923] | 0.918 [0.903, 0.924] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | split2 |  | 0.946 [0.940, 0.974] | 0.955 [0.942, 0.981] | 0.013 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n17 | split4 |  | 1.046 [1.041, 1.059] | 1.054 [1.032, 1.066] | 0.014 / 0.014 | 0.001 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n17 | selected | yes | 0.873 [0.857, 0.877] | 0.861 [0.855, 0.878] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.689 / 0.824 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | legacy_fused |  | 2.347 [2.330, 2.366] | 2.339 [2.291, 2.392] | 1.612 / 1.974 | 0.031 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1024 | product1 |  | 0.955 [0.951, 0.960] | 0.952 [0.930, 0.959] | 0.659 / 0.767 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | product2 |  | 0.954 [0.950, 0.959] | 0.952 [0.939, 0.981] | 0.658 / 0.811 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | product4 |  | 0.992 [0.986, 0.997] | 0.989 [0.960, 0.995] | 0.683 / 0.807 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | seeded2 |  | 0.952 [0.945, 0.955] | 0.950 [0.927, 0.957] | 0.657 / 0.780 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | split1 |  | 0.953 [0.948, 0.960] | 0.952 [0.930, 0.962] | 0.657 / 0.781 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | split2 |  | 0.954 [0.949, 0.962] | 0.953 [0.941, 1.028] | 0.658 / 0.851 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n1024 | split4 |  | 0.985 [0.980, 0.994] | 0.980 [0.955, 0.985] | 0.678 / 0.802 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1024 | selected | yes | 0.952 [0.946, 0.957] | 0.952 [0.926, 0.959] | 0.657 / 0.781 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 44.002 / 45.041 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | legacy_fused |  | 2.362 [2.348, 2.374] | 2.365 [2.326, 2.383] | 104.200 / 106.697 | 2.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n65536 | product1 |  | 0.949 [0.944, 0.952] | 0.950 [0.931, 0.959] | 41.779 / 42.966 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | product2 |  | 0.950 [0.939, 0.953] | 0.947 [0.931, 0.957] | 41.761 / 42.660 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | product4 |  | 0.994 [0.989, 0.997] | 0.994 [0.977, 1.004] | 43.696 / 44.942 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | seeded2 |  | 0.951 [0.945, 0.953] | 0.952 [0.933, 0.965] | 41.791 / 42.964 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | split1 |  | 0.948 [0.934, 0.954] | 0.952 [0.935, 0.965] | 41.757 / 42.692 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | split2 |  | 0.951 [0.948, 0.955] | 0.951 [0.941, 1.001] | 41.841 / 42.902 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | split4 |  | 0.983 [0.976, 0.987] | 0.984 [0.968, 0.994] | 43.243 / 43.930 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n65536 | selected | yes | 0.946 [0.943, 0.954] | 0.951 [0.930, 0.957] | 41.800 / 42.764 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 719.914 / 747.170 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | legacy_fused |  | 2.309 [2.304, 2.311] | 2.279 [2.225, 2.324] | 1661.630 / 1693.725 | 32.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | signed16_n1048576 | product1 |  | 0.947 [0.945, 0.949] | 0.956 [0.946, 0.979] | 681.771 / 721.616 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | product2 |  | 0.943 [0.942, 0.947] | 0.945 [0.938, 0.993] | 679.719 / 716.816 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | product4 |  | 0.982 [0.980, 0.984] | 0.977 [0.958, 0.999] | 707.156 / 731.511 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | seeded2 |  | 0.943 [0.942, 0.946] | 0.946 [0.929, 0.977] | 679.396 / 708.307 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | split1 |  | 0.947 [0.945, 0.948] | 0.949 [0.943, 0.988] | 681.685 / 717.897 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | split2 |  | 0.946 [0.945, 0.951] | 0.953 [0.942, 1.018] | 681.312 / 719.192 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | signed16_n1048576 | split4 |  | 0.973 [0.972, 0.977] | 0.977 [0.969, 0.997] | 701.107 / 733.038 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | signed16_n1048576 | selected | yes | 0.944 [0.942, 0.946] | 0.945 [0.940, 0.977] | 679.630 / 713.199 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | legacy_fused |  | 1.001 [0.996, 1.005] | 1.002 [0.995, 1.009] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | product1 |  | 0.970 [0.967, 0.975] | 0.969 [0.961, 0.975] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | product2 |  | 0.970 [0.961, 0.974] | 0.970 [0.956, 0.976] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | product4 |  | 1.092 [1.087, 1.098] | 1.091 [1.084, 1.104] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1 | seeded2 |  | 0.872 [0.869, 0.898] | 0.879 [0.869, 0.893] | 0.002 / 0.002 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1 | split1 |  | 0.999 [0.995, 1.005] | 1.000 [0.994, 1.016] | 0.003 / 0.003 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n1 | split2 |  | 1.091 [1.086, 1.095] | 1.090 [1.081, 1.101] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1 | split4 |  | 1.264 [1.255, 1.271] | 1.254 [1.243, 1.273] | 0.003 / 0.003 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1 | selected | yes | 0.900 [0.873, 0.931] | 0.900 [0.879, 0.933] | 0.002 / 0.002 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | legacy_fused |  | 1.083 [1.078, 1.088] | 1.085 [1.072, 1.100] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | product1 |  | 1.000 [0.991, 1.004] | 1.000 [0.989, 1.012] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n3 | product2 |  | 1.000 [0.994, 1.003] | 1.000 [0.983, 1.006] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | product4 |  | 0.980 [0.975, 0.983] | 0.979 [0.959, 0.988] | 0.004 / 0.004 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | seeded2 |  | 0.867 [0.861, 0.873] | 0.869 [0.858, 0.880] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n3 | split1 |  | 1.002 [0.994, 1.007] | 0.999 [0.986, 1.020] | 0.004 / 0.004 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n3 | split2 |  | 1.141 [1.137, 1.145] | 1.140 [1.127, 1.145] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | split4 |  | 1.098 [1.095, 1.103] | 1.097 [1.086, 1.106] | 0.004 / 0.004 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n3 | selected | yes | 0.869 [0.865, 0.873] | 0.870 [0.861, 0.885] | 0.003 / 0.003 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.007 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | legacy_fused |  | 1.120 [1.114, 1.132] | 1.120 [1.110, 1.141] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n7 | product1 |  | 0.907 [0.903, 0.915] | 0.916 [0.907, 0.921] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | product2 |  | 0.876 [0.869, 0.883] | 0.883 [0.870, 0.903] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | product4 |  | 0.928 [0.922, 0.937] | 0.927 [0.920, 0.944] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | seeded2 |  | 0.820 [0.815, 0.825] | 0.828 [0.820, 0.839] | 0.005 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | split1 |  | 0.908 [0.905, 0.916] | 0.914 [0.910, 0.930] | 0.006 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | split2 |  | 0.956 [0.950, 0.959] | 0.957 [0.952, 0.970] | 0.006 / 0.007 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n7 | split4 |  | 1.100 [1.093, 1.108] | 1.100 [1.091, 1.116] | 0.007 / 0.008 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n7 | selected | yes | 0.826 [0.822, 0.832] | 0.836 [0.822, 0.846] | 0.005 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | legacy_fused |  | 1.315 [1.310, 1.324] | 1.330 [1.307, 1.340] | 0.017 / 0.017 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n16 | product1 |  | 0.937 [0.930, 0.944] | 0.940 [0.923, 0.952] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | product2 |  | 0.895 [0.889, 0.902] | 0.906 [0.891, 0.917] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | product4 |  | 0.952 [0.944, 0.955] | 0.962 [0.936, 0.969] | 0.012 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | seeded2 |  | 0.863 [0.856, 0.865] | 0.869 [0.854, 0.879] | 0.011 / 0.011 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | split1 |  | 0.917 [0.908, 0.924] | 0.924 [0.917, 0.944] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | split2 |  | 0.953 [0.949, 0.962] | 0.960 [0.945, 0.985] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n16 | split4 |  | 1.050 [1.042, 1.060] | 1.056 [1.047, 1.074] | 0.013 / 0.014 | 0.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n16 | selected | yes | 0.888 [0.880, 0.894] | 0.896 [0.883, 0.910] | 0.011 / 0.012 | 0.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | legacy_fused |  | 1.341 [1.333, 1.346] | 1.344 [1.301, 1.373] | 0.018 / 0.018 | 0.001 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n17 | product1 |  | 0.920 [0.917, 0.928] | 0.928 [0.905, 0.965] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | product2 |  | 0.895 [0.890, 0.909] | 0.902 [0.876, 0.919] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | product4 |  | 0.948 [0.944, 0.955] | 0.949 [0.932, 0.985] | 0.013 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | seeded2 |  | 0.869 [0.866, 0.879] | 0.876 [0.858, 0.892] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | split1 |  | 0.920 [0.915, 0.925] | 0.923 [0.903, 0.968] | 0.012 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | split2 |  | 0.949 [0.943, 0.954] | 0.954 [0.934, 0.977] | 0.013 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n17 | split4 |  | 1.053 [1.049, 1.057] | 1.056 [1.030, 1.071] | 0.014 / 0.015 | 0.001 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n17 | selected | yes | 0.883 [0.876, 0.887] | 0.891 [0.871, 0.919] | 0.012 / 0.012 | 0.001 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.687 / 0.705 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | legacy_fused |  | 2.337 [2.329, 2.347] | 2.336 [2.310, 2.353] | 1.611 / 1.641 | 0.031 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1024 | product1 |  | 0.958 [0.953, 0.960] | 0.957 [0.950, 0.970] | 0.657 / 0.675 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | product2 |  | 0.955 [0.951, 0.958] | 0.954 [0.938, 0.961] | 0.656 / 0.670 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | product4 |  | 0.993 [0.990, 0.998] | 0.995 [0.981, 1.013] | 0.683 / 0.701 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | two_limb_mac | full128_n1024 | seeded2 |  | 0.951 [0.947, 0.955] | 0.954 [0.944, 0.973] | 0.654 / 0.677 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | split1 |  | 0.955 [0.950, 0.957] | 0.952 [0.938, 0.958] | 0.655 / 0.673 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | split2 |  | 0.955 [0.950, 0.958] | 0.955 [0.944, 0.964] | 0.656 / 0.668 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | split4 |  | 0.984 [0.980, 0.986] | 0.984 [0.967, 0.992] | 0.677 / 0.689 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1024 | selected | yes | 0.953 [0.948, 0.957] | 0.955 [0.947, 0.969] | 0.656 / 0.677 | 0.031 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 43.918 / 44.497 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | legacy_fused |  | 2.366 [2.361, 2.376] | 2.357 [2.346, 2.370] | 103.986 / 105.008 | 2.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n65536 | product1 |  | 0.951 [0.947, 0.953] | 0.951 [0.946, 0.956] | 41.739 / 42.408 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | product2 |  | 0.949 [0.947, 0.953] | 0.950 [0.946, 0.954] | 41.714 / 42.235 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | product4 |  | 0.993 [0.992, 0.997] | 0.992 [0.987, 1.000] | 43.611 / 44.127 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | seeded2 |  | 0.949 [0.946, 0.952] | 0.949 [0.944, 0.954] | 41.708 / 42.239 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | split1 |  | 0.950 [0.948, 0.954] | 0.949 [0.945, 0.955] | 41.757 / 42.233 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | split2 |  | 0.952 [0.947, 0.955] | 0.951 [0.947, 0.956] | 41.774 / 42.333 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | split4 |  | 0.983 [0.979, 0.986] | 0.982 [0.977, 0.990] | 43.175 / 43.718 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n65536 | selected | yes | 0.948 [0.945, 0.952] | 0.948 [0.942, 0.953] | 41.613 / 42.183 | 2.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 720.312 / 734.006 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | legacy_fused |  | 2.307 [2.303, 2.310] | 2.286 [2.249, 2.309] | 1662.094 / 1680.120 | 32.000 | 0 / 0 | regression |
| aarch64 | two_limb_mac | full128_n1048576 | product1 |  | 0.948 [0.946, 0.950] | 0.953 [0.942, 0.967] | 682.518 / 703.144 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | product2 |  | 0.943 [0.941, 0.946] | 0.942 [0.923, 0.960] | 679.435 / 699.159 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | product4 |  | 0.982 [0.980, 0.984] | 0.982 [0.970, 1.007] | 707.430 / 729.036 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | seeded2 |  | 0.943 [0.942, 0.945] | 0.949 [0.941, 0.962] | 679.745 / 698.936 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | split1 |  | 0.948 [0.946, 0.950] | 0.959 [0.949, 0.984] | 683.102 / 705.633 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | split2 |  | 0.947 [0.945, 0.948] | 0.956 [0.936, 0.976] | 681.927 / 704.752 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | split4 |  | 0.973 [0.972, 0.975] | 0.977 [0.955, 0.992] | 701.224 / 716.869 | 32.000 | 0 / 0 | pass |
| aarch64 | two_limb_mac | full128_n1048576 | selected | yes | 0.943 [0.941, 0.946] | 0.953 [0.943, 0.981] | 679.940 / 705.090 | 32.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.330 / 0.336 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | fixed9 |  | 3.028 [2.978, 3.060] | 3.083 [3.062, 3.131] | 0.998 / 1.045 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active4_n16 | prepared_bound |  | 0.316 [0.314, 0.317] | 0.319 [0.316, 0.323] | 0.104 / 0.107 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute |  | 0.372 [0.367, 0.378] | 0.376 [0.371, 0.382] | 0.123 / 0.127 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | prepared_direct |  | 0.323 [0.313, 0.324] | 0.326 [0.318, 0.332] | 0.106 / 0.109 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute_direct |  | 0.377 [0.366, 0.378] | 0.380 [0.372, 0.387] | 0.124 / 0.128 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | public_bound4 |  | 0.317 [0.315, 0.318] | 0.320 [0.317, 0.324] | 0.105 / 0.108 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | prepared_comba4 |  | 0.329 [0.320, 0.330] | 0.331 [0.323, 0.339] | 0.108 / 0.112 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute_comba4 |  | 0.378 [0.366, 0.386] | 0.379 [0.368, 0.389] | 0.125 / 0.129 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | prepared_selected | yes | 0.317 [0.314, 0.318] | 0.321 [0.317, 0.330] | 0.105 / 0.109 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n16 | validate_execute_selected | yes | 0.374 [0.368, 0.381] | 0.380 [0.373, 0.385] | 0.123 / 0.128 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.329 / 21.783 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | fixed9 |  | 3.077 [3.021, 3.100] | 3.118 [3.071, 3.332] | 65.457 / 68.354 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active4_n1024 | prepared_bound |  | 0.317 [0.316, 0.319] | 0.320 [0.314, 0.327] | 6.770 / 7.064 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute |  | 0.403 [0.401, 0.405] | 0.403 [0.260, 0.408] | 8.587 / 8.796 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | prepared_direct |  | 0.321 [0.318, 0.322] | 0.323 [0.245, 0.330] | 6.828 / 7.162 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute_direct |  | 0.407 [0.405, 0.409] | 0.409 [0.292, 0.412] | 8.691 / 8.997 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | public_bound4 |  | 0.316 [0.314, 0.317] | 0.316 [0.205, 0.325] | 6.722 / 6.978 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | prepared_comba4 |  | 0.324 [0.322, 0.325] | 0.325 [0.318, 0.329] | 6.902 / 7.190 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute_comba4 |  | 0.409 [0.407, 0.411] | 0.410 [0.279, 0.417] | 8.727 / 9.006 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | prepared_selected | yes | 0.318 [0.316, 0.320] | 0.324 [0.224, 0.331] | 6.778 / 7.129 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n1024 | validate_execute_selected | yes | 0.403 [0.402, 0.405] | 0.407 [0.295, 0.410] | 8.613 / 8.929 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1397.766 / 1418.309 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | fixed9 |  | 3.115 [3.095, 3.136] | 3.130 [3.084, 3.150] | 4355.193 / 4446.001 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active4_n65536 | prepared_bound |  | 0.326 [0.324, 0.329] | 0.334 [0.327, 0.340] | 455.807 / 473.454 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute |  | 0.432 [0.431, 0.435] | 0.443 [0.434, 0.448] | 604.662 / 629.475 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | prepared_direct |  | 0.332 [0.331, 0.334] | 0.342 [0.334, 0.352] | 464.219 / 483.077 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute_direct |  | 0.438 [0.437, 0.440] | 0.445 [0.436, 0.458] | 613.104 / 634.538 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | public_bound4 |  | 0.327 [0.326, 0.332] | 0.335 [0.329, 0.339] | 458.698 / 473.281 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | prepared_comba4 |  | 0.333 [0.332, 0.336] | 0.344 [0.332, 0.359] | 466.115 / 489.119 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute_comba4 |  | 0.441 [0.439, 0.445] | 0.453 [0.447, 0.463] | 615.240 / 649.956 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | prepared_selected | yes | 0.326 [0.325, 0.328] | 0.333 [0.328, 0.342] | 456.948 / 474.840 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active4_n65536 | validate_execute_selected | yes | 0.434 [0.433, 0.436] | 0.446 [0.436, 0.455] | 606.484 / 633.252 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.935 / 0.992 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | fixed9 |  | 1.080 [1.060, 1.109] | 1.076 [1.052, 1.126] | 1.013 / 1.065 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n16 | prepared_bound |  | 1.100 [1.083, 1.111] | 1.064 [1.047, 1.117] | 1.030 / 1.060 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n16 | validate_execute |  | 1.104 [1.065, 1.117] | 1.083 [1.065, 1.132] | 1.024 / 1.075 | 0.004 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n16 | prepared_direct |  | 0.940 [0.927, 0.948] | 0.923 [0.906, 0.962] | 0.877 / 0.913 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | validate_execute_direct |  | 0.935 [0.923, 0.943] | 0.908 [0.892, 0.945] | 0.871 / 0.899 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | prepared_selected | yes | 0.934 [0.928, 0.944] | 0.914 [0.899, 0.958] | 0.874 / 0.908 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n16 | validate_execute_selected | yes | 0.937 [0.917, 0.944] | 0.917 [0.897, 0.950] | 0.875 / 0.903 | 0.004 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 60.005 / 61.996 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | fixed9 |  | 1.108 [1.075, 1.116] | 1.118 [1.072, 1.126] | 66.172 / 68.812 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n1024 | prepared_bound |  | 1.118 [1.089, 1.122] | 1.116 [1.081, 1.139] | 66.898 / 69.181 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n1024 | validate_execute |  | 1.106 [1.085, 1.117] | 1.121 [1.078, 1.139] | 66.253 / 69.101 | 0.281 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n1024 | prepared_direct |  | 0.918 [0.911, 0.921] | 0.913 [0.884, 0.925] | 54.821 / 56.398 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | validate_execute_direct |  | 0.915 [0.911, 0.921] | 0.907 [0.880, 0.934] | 54.933 / 56.428 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | prepared_selected | yes | 0.914 [0.910, 0.918] | 0.915 [0.882, 0.938] | 54.805 / 56.775 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n1024 | validate_execute_selected | yes | 0.914 [0.909, 0.918] | 0.918 [0.885, 0.933] | 54.843 / 56.692 | 0.281 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3887.666 / 4109.639 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | fixed9 |  | 1.129 [1.110, 1.134] | 1.098 [1.071, 1.131] | 4368.375 / 4469.933 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n65536 | prepared_bound |  | 1.130 [1.120, 1.139] | 1.103 [1.078, 1.134] | 4394.708 / 4493.099 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n65536 | validate_execute |  | 1.128 [1.116, 1.137] | 1.110 [1.069, 1.132] | 4385.230 / 4480.002 | 18.000 | 0 / 0 | regression |
| aarch64 | bounded_product | active9_n65536 | prepared_direct |  | 0.909 [0.903, 0.922] | 0.897 [0.874, 0.918] | 3555.249 / 3653.044 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | validate_execute_direct |  | 0.914 [0.908, 0.919] | 0.898 [0.872, 0.921] | 3554.271 / 3645.971 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | prepared_selected | yes | 0.913 [0.905, 0.920] | 0.894 [0.875, 0.918] | 3552.791 / 3646.450 | 18.000 | 0 / 0 | pass |
| aarch64 | bounded_product | active9_n65536 | validate_execute_selected | yes | 0.914 [0.906, 0.925] | 0.889 [0.869, 0.921] | 3558.271 / 3627.277 | 18.000 | 0 / 0 | pass |
