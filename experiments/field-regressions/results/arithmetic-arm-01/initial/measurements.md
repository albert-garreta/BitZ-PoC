# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | products | 16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | products | 16 | shared |  | 1.020 [1.018, 1.029] | 1.021 [1.010, 1.052] | 0.017 / 0.017 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | shared_unroll2 |  | 1.054 [1.051, 1.057] | 1.052 [1.040, 1.072] | 0.017 / 0.018 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | shared_unroll4 |  | 1.068 [1.066, 1.073] | 1.067 [1.058, 1.078] | 0.017 / 0.018 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | shared_unroll8 |  | 1.078 [1.076, 1.089] | 1.076 [1.067, 1.122] | 0.018 / 0.018 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | scalar_lanes | yes | 1.000 [0.998, 1.003] | 0.992 [0.985, 1.008] | 0.016 / 0.017 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | products | 16 | schoolbook |  | 2.670 [2.663, 2.680] | 2.671 [2.632, 2.718] | 0.043 / 0.045 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | karatsuba |  | 3.134 [3.126, 3.143] | 3.138 [3.104, 3.188] | 0.051 / 0.053 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | karatsuba_barrett |  | 1.768 [1.738, 1.775] | 1.770 [1.746, 1.795] | 0.029 / 0.030 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.860 / 0.870 | 0.047 | 0 / 0 | pass |
| aarch64 | products | 1024 | shared |  | 1.062 [1.061, 1.064] | 1.062 [1.047, 1.072] | 0.914 / 0.927 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | shared_unroll2 |  | 1.063 [1.062, 1.064] | 1.061 [1.051, 1.073] | 0.914 / 0.930 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | shared_unroll4 |  | 1.063 [1.061, 1.064] | 1.064 [1.054, 1.082] | 0.915 / 0.932 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | shared_unroll8 |  | 1.063 [1.062, 1.064] | 1.060 [1.051, 1.065] | 0.915 / 0.925 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | scalar_lanes | yes | 1.000 [0.999, 1.001] | 0.997 [0.985, 1.010] | 0.860 / 0.869 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | products | 1024 | schoolbook |  | 3.042 [3.037, 3.043] | 3.037 [2.996, 3.066] | 2.617 / 2.649 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | karatsuba |  | 3.554 [3.549, 3.556] | 3.557 [3.532, 3.587] | 3.057 / 3.089 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | karatsuba_barrett |  | 1.865 [1.864, 1.870] | 1.867 [1.827, 1.878] | 1.609 / 1.628 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 65536 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 55.006 / 56.919 | 3.000 | 0 / 0 | pass |
| aarch64 | products | 65536 | shared |  | 1.067 [1.061, 1.070] | 1.067 [1.054, 1.074] | 58.574 / 60.615 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | shared_unroll2 |  | 1.066 [1.059, 1.068] | 1.063 [1.049, 1.073] | 58.433 / 60.599 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | shared_unroll4 |  | 1.067 [1.057, 1.068] | 1.067 [1.047, 1.086] | 58.615 / 60.150 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | shared_unroll8 |  | 1.068 [1.062, 1.071] | 1.063 [1.050, 1.075] | 58.721 / 60.323 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | scalar_lanes | yes | 1.000 [0.996, 1.003] | 0.999 [0.985, 1.007] | 54.954 / 56.957 | 3.000 | 0 / 0 | pass |
| aarch64 | products | 65536 | schoolbook |  | 3.062 [3.029, 3.067] | 3.030 [3.000, 3.065] | 168.379 / 171.572 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | karatsuba |  | 3.585 [3.554, 3.590] | 3.568 [3.509, 3.665] | 197.403 / 201.744 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | karatsuba_barrett |  | 1.903 [1.897, 1.909] | 1.901 [1.871, 1.912] | 105.050 / 107.682 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 903.406 / 945.453 | 48.000 | 0 / 0 | pass |
| aarch64 | products | 1048576 | shared |  | 1.061 [1.058, 1.064] | 1.057 [1.022, 1.078] | 958.339 / 1001.672 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | shared_unroll2 |  | 1.056 [1.052, 1.061] | 1.049 [1.027, 1.121] | 954.510 / 1001.396 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | shared_unroll4 |  | 1.061 [1.057, 1.063] | 1.059 [1.036, 1.134] | 957.615 / 1027.718 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | shared_unroll8 |  | 1.060 [1.055, 1.064] | 1.069 [1.008, 1.127] | 957.484 / 1030.110 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | scalar_lanes | yes | 1.001 [0.997, 1.003] | 0.997 [0.926, 1.030] | 902.766 / 943.425 | 48.000 | 0 / 0 | inconclusive |
| aarch64 | products | 1048576 | schoolbook |  | 3.021 [3.006, 3.026] | 2.972 [2.620, 3.027] | 2722.677 / 2804.172 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | karatsuba |  | 3.536 [3.519, 3.541] | 3.473 [3.109, 3.526] | 3185.766 / 3272.950 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | karatsuba_barrett |  | 1.905 [1.894, 1.908] | 1.890 [1.675, 1.908] | 1713.875 / 1776.149 | 48.000 | 0 / 0 | regression |
| aarch64 | chain | 16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.060 / 0.062 | 0.000 | 0 / 0 | pass |
| aarch64 | chain | 16 | shared |  | 1.001 [0.991, 1.005] | 1.008 [0.992, 1.022] | 0.060 / 0.062 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | chain | 16 | scalar_lanes | yes | 0.995 [0.989, 0.996] | 0.996 [0.989, 1.008] | 0.059 / 0.061 | 0.000 | 0 / 0 | pass |
| aarch64 | chain | 16 | schoolbook |  | 1.881 [1.868, 1.883] | 1.877 [1.858, 1.913] | 0.112 / 0.116 | 0.000 | 0 / 0 | regression |
| aarch64 | chain | 16 | karatsuba |  | 1.861 [1.846, 1.867] | 1.851 [1.832, 1.885] | 0.111 / 0.114 | 0.000 | 0 / 0 | regression |
| aarch64 | chain | 16 | karatsuba_barrett |  | 1.369 [1.359, 1.377] | 1.368 [1.359, 1.377] | 0.082 / 0.084 | 0.000 | 0 / 0 | regression |
| aarch64 | chain | 1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8.056 / 8.297 | 0.016 | 0 / 0 | pass |
| aarch64 | chain | 1024 | shared |  | 0.989 [0.988, 0.989] | 0.987 [0.982, 1.008] | 7.965 / 8.204 | 0.016 | 0 / 0 | pass |
| aarch64 | chain | 1024 | scalar_lanes | yes | 0.989 [0.988, 0.990] | 0.991 [0.985, 1.002] | 7.970 / 8.218 | 0.016 | 0 / 0 | pass |
| aarch64 | chain | 1024 | schoolbook |  | 1.257 [1.256, 1.258] | 1.260 [1.252, 1.284] | 10.130 / 10.445 | 0.016 | 0 / 0 | regression |
| aarch64 | chain | 1024 | karatsuba |  | 1.094 [1.093, 1.097] | 1.092 [1.090, 1.099] | 8.822 / 9.082 | 0.016 | 0 / 0 | regression |
| aarch64 | chain | 1024 | karatsuba_barrett |  | 1.225 [1.224, 1.230] | 1.227 [1.222, 1.238] | 9.883 / 10.166 | 0.016 | 0 / 0 | regression |
| aarch64 | chain | 65536 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 517.995 / 534.872 | 1.000 | 0 / 0 | pass |
| aarch64 | chain | 65536 | shared |  | 0.988 [0.983, 0.990] | 0.987 [0.963, 0.990] | 512.060 / 526.762 | 1.000 | 0 / 0 | pass |
| aarch64 | chain | 65536 | scalar_lanes | yes | 0.987 [0.984, 0.989] | 0.983 [0.959, 0.989] | 510.763 / 526.346 | 1.000 | 0 / 0 | pass |
| aarch64 | chain | 65536 | schoolbook |  | 1.248 [1.246, 1.250] | 1.246 [1.214, 1.258] | 646.708 / 665.348 | 1.000 | 0 / 0 | regression |
| aarch64 | chain | 65536 | karatsuba |  | 1.095 [1.090, 1.097] | 1.090 [1.066, 1.103] | 566.656 / 583.048 | 1.000 | 0 / 0 | regression |
| aarch64 | chain | 65536 | karatsuba_barrett |  | 1.228 [1.225, 1.233] | 1.226 [1.203, 1.236] | 637.086 / 655.893 | 1.000 | 0 / 0 | regression |
| aarch64 | chain | 1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 8274.251 / 8517.073 | 16.000 | 0 / 0 | pass |
| aarch64 | chain | 1048576 | shared |  | 0.988 [0.985, 0.989] | 0.987 [0.976, 1.007] | 8175.188 / 8437.902 | 16.000 | 0 / 0 | pass |
| aarch64 | chain | 1048576 | scalar_lanes | yes | 0.985 [0.983, 0.986] | 0.986 [0.975, 0.998] | 8151.749 / 8404.089 | 16.000 | 0 / 0 | pass |
| aarch64 | chain | 1048576 | schoolbook |  | 1.248 [1.245, 1.250] | 1.244 [1.232, 1.259] | 10331.188 / 10626.550 | 16.000 | 0 / 0 | regression |
| aarch64 | chain | 1048576 | karatsuba |  | 1.096 [1.094, 1.098] | 1.095 [1.087, 1.105] | 9075.604 / 9336.250 | 16.000 | 0 / 0 | regression |
| aarch64 | chain | 1048576 | karatsuba_barrett |  | 1.233 [1.232, 1.235] | 1.235 [1.222, 1.246] | 10215.084 / 10493.337 | 16.000 | 0 / 0 | regression |
| aarch64 | square | 16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.040 / 0.041 | 0.000 | 0 / 0 | pass |
| aarch64 | square | 16 | shared | yes | 0.342 [0.342, 0.342] | 0.342 [0.339, 0.346] | 0.014 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | square | 1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.379 / 2.461 | 0.031 | 0 / 0 | pass |
| aarch64 | square | 1024 | shared | yes | 0.276 [0.276, 0.277] | 0.277 [0.276, 0.281] | 0.657 / 0.694 | 0.031 | 0 / 0 | pass |
| aarch64 | square | 65536 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 151.819 / 159.308 | 2.000 | 0 / 0 | pass |
| aarch64 | square | 65536 | shared | yes | 0.273 [0.272, 0.274] | 0.275 [0.270, 0.278] | 41.435 / 43.230 | 2.000 | 0 / 0 | pass |
| aarch64 | square | 1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2460.771 / 2638.381 | 32.000 | 0 / 0 | pass |
| aarch64 | square | 1048576 | shared | yes | 0.280 [0.279, 0.281] | 0.283 [0.279, 0.289] | 690.875 / 739.488 | 32.000 | 0 / 0 | pass |
| aarch64 | inverse | 1 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.053 / 1.084 | 0.000 | 0 / 0 | pass |
| aarch64 | inverse | 1 | shared | yes | 0.418 [0.417, 0.418] | 0.418 [0.415, 0.419] | 0.440 / 0.452 | 0.000 | 0 / 0 | pass |
| aarch64 | inverse | 16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 16.829 / 17.379 | 0.000 | 0 / 0 | pass |
| aarch64 | inverse | 16 | shared | yes | 0.412 [0.412, 0.413] | 0.414 [0.409, 0.416] | 6.942 / 7.155 | 0.000 | 0 / 0 | pass |
| aarch64 | inverse | 1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1077.943 / 1108.435 | 0.031 | 0 / 0 | pass |
| aarch64 | inverse | 1024 | shared | yes | 0.413 [0.412, 0.413] | 0.413 [0.410, 0.414] | 444.932 / 457.680 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed_prepare | zero | existing_prepare |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.001 / 0.001 | 0.000 | 0 / 0 | unmeasured (diagnostic) |
| aarch64 | fixed_prepare | half | existing_prepare |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.001 / 0.001 | 0.000 | 0 / 0 | unmeasured (diagnostic) |
| aarch64 | fixed_prepare | full | existing_prepare |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.001 / 0.001 | 0.000 | 0 / 0 | unmeasured (diagnostic) |
| aarch64 | fixed | zero_n16 | flock |  | 1.061 [1.060, 1.062] | 1.062 [1.056, 1.064] | 0.015 / 0.015 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | zero_n16 | shared |  | 1.072 [1.071, 1.073] | 1.073 [1.065, 1.076] | 0.015 / 0.016 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | zero_n16 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.014 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n16 | specialized | yes | 0.353 [0.351, 0.399] | 0.354 [0.351, 0.393] | 0.005 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n16 | scalar_lanes |  | 1.089 [1.088, 1.089] | 1.089 [1.082, 1.093] | 0.016 / 0.016 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | zero_n1024 | flock |  | 1.113 [1.112, 1.114] | 1.114 [1.107, 1.117] | 0.825 / 0.841 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | zero_n1024 | shared |  | 1.166 [1.162, 1.166] | 1.166 [1.151, 1.170] | 0.864 / 0.881 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | zero_n1024 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.742 / 0.757 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | zero_n1024 | specialized | yes | 0.273 [0.249, 0.307] | 0.443 [0.271, 0.565] | 0.195 / 0.413 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | zero_n1024 | scalar_lanes |  | 1.113 [1.111, 1.114] | 1.114 [1.104, 1.119] | 0.826 / 0.845 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | zero_n65536 | flock |  | 1.114 [1.105, 1.116] | 1.104 [1.095, 1.116] | 52.849 / 54.683 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | zero_n65536 | shared |  | 1.146 [1.119, 1.149] | 1.125 [1.106, 1.147] | 54.215 / 55.505 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | zero_n65536 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 47.511 / 49.749 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n65536 | specialized | yes | 0.239 [0.200, 0.335] | 0.348 [0.227, 0.430] | 11.130 / 19.139 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n65536 | scalar_lanes |  | 1.113 [1.104, 1.118] | 1.108 [1.093, 1.116] | 52.824 / 54.882 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | zero_n1048576 | flock |  | 1.110 [1.098, 1.115] | 1.101 [1.032, 1.128] | 867.568 / 967.695 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | zero_n1048576 | shared |  | 1.106 [1.096, 1.114] | 1.107 [1.011, 1.215] | 867.495 / 996.072 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | zero_n1048576 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 785.182 / 894.833 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n1048576 | specialized | yes | 0.195 [0.194, 0.197] | 0.196 [0.179, 0.201] | 152.938 / 170.945 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n1048576 | scalar_lanes |  | 1.105 [1.092, 1.113] | 1.077 [1.024, 1.121] | 865.198 / 952.774 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n16 | flock |  | 1.061 [1.059, 1.062] | 1.060 [1.055, 1.066] | 0.015 / 0.015 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n16 | shared |  | 1.071 [1.071, 1.072] | 1.069 [1.059, 1.073] | 0.015 / 0.016 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n16 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.014 / 0.015 | 0.000 | 0 / 0 | pass |
| aarch64 | fixed | half_n16 | specialized | yes | 1.106 [1.106, 1.110] | 1.105 [1.099, 1.110] | 0.016 / 0.016 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n16 | scalar_lanes |  | 1.089 [1.088, 1.091] | 1.089 [1.081, 1.097] | 0.016 / 0.016 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n1024 | flock |  | 1.113 [1.112, 1.114] | 1.113 [1.106, 1.116] | 0.826 / 0.834 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | half_n1024 | shared |  | 1.166 [1.162, 1.167] | 1.164 [1.156, 1.169] | 0.865 / 0.874 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | half_n1024 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.742 / 0.750 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | half_n1024 | specialized | yes | 1.272 [1.268, 1.278] | 1.283 [1.272, 1.287] | 0.946 / 0.962 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | half_n1024 | scalar_lanes |  | 1.113 [1.111, 1.114] | 1.113 [1.109, 1.118] | 0.826 / 0.835 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | half_n65536 | flock |  | 1.114 [1.099, 1.114] | 1.113 [1.085, 1.115] | 52.707 / 54.112 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n65536 | shared |  | 1.145 [1.109, 1.147] | 1.153 [1.090, 1.158] | 54.095 / 54.781 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n65536 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 47.351 / 49.337 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | half_n65536 | specialized | yes | 1.168 [1.139, 1.187] | 1.185 [1.133, 1.212] | 55.776 / 57.183 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n65536 | scalar_lanes |  | 1.112 [1.101, 1.114] | 1.113 [1.088, 1.115] | 52.687 / 54.130 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n1048576 | flock |  | 1.111 [1.095, 1.114] | 1.109 [1.081, 1.121] | 857.177 / 879.558 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n1048576 | shared |  | 1.111 [1.101, 1.114] | 1.111 [1.099, 1.127] | 858.026 / 885.743 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n1048576 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 771.401 / 806.644 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | half_n1048576 | specialized | yes | 1.127 [1.118, 1.132] | 1.136 [1.105, 1.186] | 873.849 / 902.053 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | half_n1048576 | scalar_lanes |  | 1.109 [1.098, 1.113] | 1.107 [1.086, 1.121] | 857.542 / 880.659 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n16 | flock |  | 1.061 [1.060, 1.061] | 1.059 [1.054, 1.071] | 0.015 / 0.015 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n16 | shared |  | 1.072 [1.071, 1.072] | 1.070 [1.065, 1.074] | 0.015 / 0.016 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n16 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.014 / 0.015 | 0.000 | 0 / 0 | pass |
| aarch64 | fixed | full_n16 | specialized | yes | 1.039 [1.038, 1.040] | 1.039 [1.036, 1.046] | 0.015 / 0.015 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n16 | scalar_lanes |  | 1.088 [1.087, 1.089] | 1.088 [1.081, 1.099] | 0.016 / 0.016 | 0.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n1024 | flock |  | 1.113 [1.112, 1.114] | 1.112 [1.109, 1.116] | 0.825 / 0.831 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | full_n1024 | shared |  | 1.166 [1.163, 1.167] | 1.162 [1.159, 1.170] | 0.864 / 0.868 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | full_n1024 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.741 / 0.748 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | full_n1024 | specialized | yes | 1.001 [1.001, 1.002] | 1.003 [0.999, 1.005] | 0.742 / 0.751 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | full_n1024 | scalar_lanes |  | 1.113 [1.113, 1.114] | 1.114 [1.111, 1.120] | 0.825 / 0.834 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | full_n65536 | flock |  | 1.114 [1.102, 1.115] | 1.106 [1.084, 1.134] | 52.787 / 54.199 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n65536 | shared |  | 1.146 [1.113, 1.149] | 1.131 [1.093, 1.155] | 54.144 / 54.739 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n65536 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 47.348 / 49.351 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | full_n65536 | specialized | yes | 0.999 [0.997, 1.001] | 1.000 [0.984, 1.010] | 47.350 / 49.401 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | full_n65536 | scalar_lanes |  | 1.114 [1.102, 1.115] | 1.099 [1.087, 1.125] | 52.768 / 54.310 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n1048576 | flock |  | 1.112 [1.096, 1.115] | 1.107 [1.089, 1.118] | 859.375 / 888.623 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n1048576 | shared |  | 1.113 [1.097, 1.114] | 1.109 [1.089, 1.121] | 858.885 / 888.663 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n1048576 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 771.213 / 810.459 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | full_n1048576 | specialized | yes | 1.001 [0.994, 1.002] | 0.995 [0.985, 1.012] | 771.255 / 806.680 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | full_n1048576 | scalar_lanes |  | 1.112 [1.097, 1.114] | 1.104 [1.087, 1.120] | 858.401 / 886.513 | 32.000 | 0 / 0 | regression |
| aarch64 | butterfly | zero_n16 | flock |  | 1.011 [1.008, 1.011] | 1.015 [1.008, 1.022] | 0.021 / 0.021 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n16 | shared |  | 1.011 [1.009, 1.011] | 1.012 [1.007, 1.023] | 0.021 / 0.021 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n16 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n16 | specialized | yes | 0.611 [0.606, 0.612] | 0.614 [0.608, 0.621] | 0.013 / 0.013 | 0.001 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n16 | scalar_lanes |  | 1.011 [1.009, 1.011] | 1.012 [1.007, 1.026] | 0.021 / 0.021 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n1024 | flock |  | 1.049 [1.048, 1.050] | 1.047 [1.043, 1.057] | 1.157 / 1.175 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | zero_n1024 | shared |  | 1.049 [1.048, 1.049] | 1.051 [1.044, 1.058] | 1.157 / 1.175 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | zero_n1024 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.103 / 1.121 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n1024 | specialized | yes | 0.534 [0.533, 0.539] | 0.577 [0.534, 0.642] | 0.590 / 0.657 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n1024 | scalar_lanes |  | 1.049 [1.048, 1.050] | 1.048 [1.044, 1.055] | 1.157 / 1.173 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | zero_n65536 | flock |  | 1.050 [1.047, 1.052] | 1.050 [1.028, 1.083] | 74.799 / 79.832 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | zero_n65536 | shared |  | 1.050 [1.046, 1.052] | 1.053 [1.023, 1.076] | 74.803 / 78.734 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | zero_n65536 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 71.282 / 74.094 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n65536 | specialized | yes | 0.634 [0.536, 0.637] | 0.651 [0.609, 0.699] | 44.788 / 49.299 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n65536 | scalar_lanes |  | 1.051 [1.049, 1.055] | 1.051 [1.017, 1.100] | 74.786 / 79.832 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | zero_n1048576 | flock |  | 1.047 [1.041, 1.048] | 1.040 [1.023, 1.056] | 1199.010 / 1251.203 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | zero_n1048576 | shared |  | 1.047 [1.042, 1.048] | 1.039 [1.027, 1.073] | 1198.734 / 1251.175 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | zero_n1048576 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1145.510 / 1208.634 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n1048576 | specialized | yes | 0.955 [0.945, 0.957] | 0.946 [0.920, 0.968] | 1095.255 / 1130.970 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n1048576 | scalar_lanes |  | 1.047 [1.039, 1.049] | 1.031 [1.014, 1.049] | 1198.932 / 1250.410 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | half_n16 | flock |  | 1.011 [1.009, 1.011] | 1.011 [0.994, 1.018] | 0.021 / 0.021 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n16 | shared |  | 1.011 [1.010, 1.011] | 1.011 [1.003, 1.021] | 0.021 / 0.021 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n16 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | butterfly | half_n16 | specialized | yes | 1.182 [1.181, 1.184] | 1.174 [1.164, 1.185] | 0.024 / 0.025 | 0.001 | 0 / 0 | regression |
| aarch64 | butterfly | half_n16 | scalar_lanes |  | 1.011 [1.010, 1.012] | 1.010 [1.001, 1.019] | 0.021 / 0.021 | 0.001 | 0 / 0 | regression |
| aarch64 | butterfly | half_n1024 | flock |  | 1.048 [1.048, 1.049] | 1.048 [1.040, 1.054] | 1.157 / 1.171 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | half_n1024 | shared |  | 1.049 [1.047, 1.049] | 1.044 [1.037, 1.052] | 1.157 / 1.170 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | half_n1024 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.103 / 1.120 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | half_n1024 | specialized | yes | 1.053 [1.052, 1.056] | 1.055 [1.042, 1.063] | 1.162 / 1.185 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | half_n1024 | scalar_lanes |  | 1.049 [1.048, 1.050] | 1.043 [1.033, 1.051] | 1.157 / 1.171 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | half_n65536 | flock |  | 1.049 [1.048, 1.051] | 1.052 [1.033, 1.078] | 74.254 / 77.887 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | half_n65536 | shared |  | 1.050 [1.050, 1.053] | 1.053 [1.046, 1.068] | 74.284 / 77.596 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | half_n65536 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 70.711 / 73.890 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | half_n65536 | specialized | yes | 1.087 [1.085, 1.088] | 1.081 [0.999, 1.106] | 76.868 / 79.485 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | half_n65536 | scalar_lanes |  | 1.049 [1.046, 1.050] | 1.053 [1.040, 1.064] | 74.305 / 77.219 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | half_n1048576 | flock |  | 1.045 [1.040, 1.048] | 1.047 [1.033, 1.057] | 1198.177 / 1249.217 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | half_n1048576 | shared |  | 1.046 [1.043, 1.048] | 1.038 [1.021, 1.054] | 1200.406 / 1249.163 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | half_n1048576 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1147.734 / 1194.094 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | half_n1048576 | specialized | yes | 1.081 [1.079, 1.082] | 1.073 [1.058, 1.083] | 1239.177 / 1288.841 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | half_n1048576 | scalar_lanes |  | 1.045 [1.041, 1.049] | 1.043 [1.027, 1.059] | 1200.505 / 1249.428 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n16 | flock |  | 1.011 [1.009, 1.011] | 1.009 [0.997, 1.015] | 0.021 / 0.021 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n16 | shared |  | 1.011 [1.009, 1.011] | 1.010 [0.995, 1.015] | 0.021 / 0.021 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n16 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.021 | 0.001 | 0 / 0 | pass |
| aarch64 | butterfly | full_n16 | specialized | yes | 1.028 [1.026, 1.028] | 1.025 [1.011, 1.031] | 0.021 / 0.021 | 0.001 | 0 / 0 | regression |
| aarch64 | butterfly | full_n16 | scalar_lanes |  | 1.011 [1.009, 1.011] | 1.013 [1.002, 1.017] | 0.021 / 0.021 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n1024 | flock |  | 1.048 [1.047, 1.049] | 1.049 [1.043, 1.065] | 1.157 / 1.171 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | full_n1024 | shared |  | 1.049 [1.047, 1.049] | 1.049 [1.044, 1.056] | 1.156 / 1.168 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | full_n1024 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.103 / 1.115 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | full_n1024 | specialized | yes | 1.001 [1.000, 1.002] | 1.002 [0.998, 1.014] | 1.104 / 1.118 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n1024 | scalar_lanes |  | 1.049 [1.047, 1.050] | 1.048 [1.045, 1.058] | 1.157 / 1.169 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | full_n65536 | flock |  | 1.051 [1.049, 1.053] | 1.053 [1.009, 1.077] | 74.705 / 83.880 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n65536 | shared |  | 1.051 [1.049, 1.052] | 1.054 [1.003, 1.076] | 74.686 / 83.435 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n65536 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 70.958 / 82.569 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n65536 | specialized | yes | 0.999 [0.998, 1.001] | 1.004 [0.983, 1.030] | 71.244 / 82.495 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n65536 | scalar_lanes |  | 1.051 [1.050, 1.054] | 1.059 [1.007, 1.073] | 74.563 / 83.523 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n1048576 | flock |  | 1.047 [1.040, 1.048] | 1.046 [1.027, 1.055] | 1200.427 / 1262.245 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n1048576 | shared |  | 1.047 [1.045, 1.048] | 1.048 [1.034, 1.083] | 1201.646 / 1270.135 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n1048576 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1147.312 / 1204.335 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n1048576 | specialized | yes | 0.999 [0.995, 1.001] | 1.006 [0.993, 1.069] | 1148.630 / 1212.294 | 64.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n1048576 | scalar_lanes |  | 1.045 [1.042, 1.048] | 1.045 [1.029, 1.057] | 1198.568 / 1255.018 | 64.000 | 0 / 0 | regression |
| aarch64 | gf_dot | 16 | f2z_wide |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | gf_dot | 16 | flock_eager |  | 1.153 [1.152, 1.344] | 1.153 [1.138, 1.339] | 0.015 / 0.018 | 0.000 | 0 / 0 | regression |
| aarch64 | gf_dot | 16 | shared_wide1 | yes | 1.000 [0.999, 1.001] | 0.996 [0.986, 1.003] | 0.013 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | gf_dot | 16 | shared_wide2 |  | 1.033 [1.032, 1.034] | 1.030 [1.020, 1.035] | 0.014 / 0.014 | 0.000 | 0 / 0 | regression |
| aarch64 | gf_dot | 16 | shared_wide4 |  | 1.049 [1.048, 1.050] | 1.051 [1.035, 1.056] | 0.014 / 0.014 | 0.000 | 0 / 0 | regression |
| aarch64 | gf_dot | 16 | shared_wide8 |  | 1.152 [1.151, 1.153] | 1.146 [1.137, 1.154] | 0.015 / 0.015 | 0.000 | 0 / 0 | regression |
| aarch64 | gf_dot | 1024 | f2z_wide |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.696 / 0.703 | 0.031 | 0 / 0 | pass |
| aarch64 | gf_dot | 1024 | flock_eager |  | 1.236 [1.235, 1.240] | 1.233 [1.231, 1.260] | 0.861 / 0.868 | 0.031 | 0 / 0 | regression |
| aarch64 | gf_dot | 1024 | shared_wide1 | yes | 1.000 [1.000, 1.001] | 1.000 [0.996, 1.009] | 0.696 / 0.703 | 0.031 | 0 / 0 | pass |
| aarch64 | gf_dot | 1024 | shared_wide2 |  | 1.001 [1.000, 1.001] | 1.003 [0.996, 1.014] | 0.696 / 0.702 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | gf_dot | 1024 | shared_wide4 |  | 1.001 [1.001, 1.002] | 1.001 [0.996, 1.008] | 0.696 / 0.704 | 0.031 | 0 / 0 | pass |
| aarch64 | gf_dot | 1024 | shared_wide8 |  | 1.060 [1.059, 1.061] | 1.065 [1.058, 1.073] | 0.737 / 0.746 | 0.031 | 0 / 0 | regression |
| aarch64 | gf_dot | 65536 | f2z_wide |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 44.137 / 44.745 | 2.000 | 0 / 0 | pass |
| aarch64 | gf_dot | 65536 | flock_eager |  | 1.239 [1.238, 1.240] | 1.237 [1.223, 1.250] | 54.684 / 55.672 | 2.000 | 0 / 0 | regression |
| aarch64 | gf_dot | 65536 | shared_wide1 | yes | 1.000 [0.999, 1.001] | 1.000 [0.990, 1.002] | 44.135 / 44.718 | 2.000 | 0 / 0 | pass |
| aarch64 | gf_dot | 65536 | shared_wide2 |  | 1.000 [0.999, 1.001] | 0.999 [0.994, 1.006] | 44.128 / 44.858 | 2.000 | 0 / 0 | pass |
| aarch64 | gf_dot | 65536 | shared_wide4 |  | 1.000 [0.999, 1.001] | 1.000 [0.997, 1.005] | 44.123 / 44.885 | 2.000 | 0 / 0 | pass |
| aarch64 | gf_dot | 65536 | shared_wide8 |  | 1.060 [1.060, 1.064] | 1.061 [1.054, 1.073] | 46.788 / 47.893 | 2.000 | 0 / 0 | regression |
| aarch64 | gf_dot | 1048576 | f2z_wide |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 720.734 / 734.176 | 32.000 | 0 / 0 | pass |
| aarch64 | gf_dot | 1048576 | flock_eager |  | 1.229 [1.226, 1.231] | 1.219 [1.208, 1.233] | 885.656 / 893.708 | 32.000 | 0 / 0 | regression |
| aarch64 | gf_dot | 1048576 | shared_wide1 | yes | 0.995 [0.994, 0.997] | 0.996 [0.981, 1.002] | 717.813 / 727.770 | 32.000 | 0 / 0 | pass |
| aarch64 | gf_dot | 1048576 | shared_wide2 |  | 0.996 [0.995, 0.997] | 0.989 [0.982, 1.000] | 717.992 / 727.245 | 32.000 | 0 / 0 | pass |
| aarch64 | gf_dot | 1048576 | shared_wide4 |  | 0.996 [0.994, 0.998] | 0.994 [0.986, 1.008] | 718.094 / 729.780 | 32.000 | 0 / 0 | pass |
| aarch64 | gf_dot | 1048576 | shared_wide8 |  | 1.055 [1.053, 1.056] | 1.051 [1.040, 1.066] | 760.531 / 769.251 | 32.000 | 0 / 0 | regression |
| aarch64 | prime_setup | 100 | existing_contexts |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.220 / 0.222 | 0.000 | 0 / 0 | unmeasured (diagnostic) |
| aarch64 | prime_setup | 128 | existing_contexts |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.209 / 0.212 | 0.000 | 0 / 0 | unmeasured (diagnostic) |
| aarch64 | prime_mul | q100_n16 | raw_ctx |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.078 / 0.079 | 0.001 | 0 / 0 | pass |
| aarch64 | prime_mul | q100_n16 | branded | yes | 1.021 [1.002, 1.024] | 1.020 [1.000, 1.025] | 0.079 / 0.080 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | prime_mul | q100_n16 | checked_per_term |  | 1.115 [1.097, 1.115] | 1.113 [1.101, 1.128] | 0.086 / 0.088 | 0.001 | 0 / 0 | regression |
| aarch64 | prime_mul | q100_n16 | configured_field |  | 1.584 [1.552, 1.585] | 1.581 [1.548, 1.590] | 0.123 / 0.124 | 0.004 | 0 / 0 | regression |
| aarch64 | prime_mul | q100_n1024 | raw_ctx |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.821 / 4.917 | 0.047 | 0 / 0 | pass |
| aarch64 | prime_mul | q100_n1024 | branded | yes | 1.000 [1.000, 1.001] | 0.997 [0.982, 1.015] | 4.822 / 4.894 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | prime_mul | q100_n1024 | checked_per_term |  | 1.116 [1.115, 1.116] | 1.115 [1.100, 1.124] | 5.377 / 5.468 | 0.047 | 0 / 0 | regression |
| aarch64 | prime_mul | q100_n1024 | configured_field |  | 1.604 [1.602, 1.611] | 1.607 [1.584, 1.631] | 7.735 / 7.893 | 0.234 | 0 / 0 | regression |
| aarch64 | prime_mul | q100_n1048576 | raw_ctx |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4993.083 / 5164.391 | 48.000 | 0 / 0 | pass |
| aarch64 | prime_mul | q100_n1048576 | branded | yes | 1.000 [0.995, 1.001] | 1.002 [0.989, 1.005] | 5002.645 / 5180.698 | 48.000 | 0 / 0 | pass |
| aarch64 | prime_mul | q100_n1048576 | checked_per_term |  | 1.112 [1.107, 1.113] | 1.109 [1.100, 1.112] | 5548.229 / 5721.761 | 48.000 | 0 / 0 | regression |
| aarch64 | prime_mul | q100_n1048576 | configured_field |  | 1.628 [1.599, 1.631] | 1.668 [1.608, 1.859] | 8141.271 / 8675.362 | 240.000 | 0 / 0 | regression |
| aarch64 | prime_mul | q128_n16 | raw_ctx |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.078 / 0.079 | 0.001 | 0 / 0 | pass |
| aarch64 | prime_mul | q128_n16 | branded | yes | 1.020 [1.001, 1.022] | 1.017 [1.001, 1.024] | 0.079 / 0.080 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | prime_mul | q128_n16 | checked_per_term |  | 1.112 [1.097, 1.115] | 1.111 [1.095, 1.114] | 0.087 / 0.087 | 0.001 | 0 / 0 | regression |
| aarch64 | prime_mul | q128_n16 | configured_field |  | 1.582 [1.553, 1.586] | 1.582 [1.551, 1.604] | 0.123 / 0.124 | 0.004 | 0 / 0 | regression |
| aarch64 | prime_mul | q128_n1024 | raw_ctx |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.834 / 5.011 | 0.047 | 0 / 0 | pass |
| aarch64 | prime_mul | q128_n1024 | branded | yes | 1.000 [0.998, 1.002] | 0.996 [0.981, 1.013] | 4.833 / 4.973 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | prime_mul | q128_n1024 | checked_per_term |  | 1.115 [1.114, 1.116] | 1.113 [1.090, 1.128] | 5.388 / 5.539 | 0.047 | 0 / 0 | regression |
| aarch64 | prime_mul | q128_n1024 | configured_field |  | 1.605 [1.603, 1.610] | 1.605 [1.571, 1.622] | 7.766 / 7.975 | 0.234 | 0 / 0 | regression |
| aarch64 | prime_mul | q128_n1048576 | raw_ctx |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 5048.417 / 5208.899 | 48.000 | 0 / 0 | pass |
| aarch64 | prime_mul | q128_n1048576 | branded | yes | 1.000 [0.997, 1.003] | 0.985 [0.955, 1.002] | 5033.145 / 5167.506 | 48.000 | 0 / 0 | pass |
| aarch64 | prime_mul | q128_n1048576 | checked_per_term |  | 1.112 [1.109, 1.115] | 1.100 [1.067, 1.115] | 5616.542 / 5758.623 | 48.000 | 0 / 0 | regression |
| aarch64 | prime_mul | q128_n1048576 | configured_field |  | 1.607 [1.595, 1.628] | 1.589 [1.567, 1.628] | 8135.208 / 8357.587 | 240.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n16 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.058 / 0.058 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_dot | q100_n16 | acc2 |  | 1.015 [1.013, 1.016] | 1.011 [0.993, 1.018] | 0.059 / 0.059 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n16 | acc4 | yes | 1.045 [1.044, 1.047] | 1.045 [1.028, 1.060] | 0.061 / 0.061 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n16 | acc8 |  | 1.096 [1.093, 1.098] | 1.096 [1.080, 1.110] | 0.063 / 0.064 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n16 | checked_per_term |  | 1.155 [1.153, 1.156] | 1.159 [1.149, 1.177] | 0.067 / 0.068 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n16 | eager_raw |  | 1.745 [1.743, 1.809] | 1.753 [1.736, 1.783] | 0.101 / 0.105 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n16 | crypto_bigint_reduce |  | 2.586 [2.583, 2.627] | 2.579 [2.544, 2.615] | 0.150 / 0.153 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n16 | convert_then_delayed |  | 2.054 [2.006, 2.061] | 2.061 [2.007, 2.091] | 0.119 / 0.121 | 0.001 | 2 / 512 | regression |
| aarch64 | prime_dot | q100_n1024 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.387 / 2.478 | 0.031 | 0 / 0 | pass |
| aarch64 | prime_dot | q100_n1024 | acc2 |  | 0.997 [0.996, 0.998] | 0.997 [0.980, 1.022] | 2.381 / 2.473 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | prime_dot | q100_n1024 | acc4 | yes | 0.987 [0.986, 0.988] | 0.988 [0.967, 1.025] | 2.357 / 2.441 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | prime_dot | q100_n1024 | acc8 |  | 1.039 [1.038, 1.040] | 1.042 [1.026, 1.059] | 2.479 / 2.570 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1024 | checked_per_term |  | 1.222 [1.217, 1.223] | 1.222 [1.195, 1.246] | 2.916 / 3.018 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1024 | eager_raw |  | 2.749 [2.747, 2.751] | 2.752 [2.705, 2.807] | 6.563 / 6.780 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1024 | crypto_bigint_reduce |  | 1.043 [1.042, 1.044] | 1.047 [1.021, 1.060] | 2.491 / 2.582 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1024 | convert_then_delayed |  | 1.212 [1.208, 1.215] | 1.210 [1.190, 1.233] | 2.892 / 2.992 | 0.062 | 2 / 32768 | regression |
| aarch64 | prime_dot | q100_n1048576 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2452.094 / 2486.047 | 32.000 | 0 / 0 | pass |
| aarch64 | prime_dot | q100_n1048576 | acc2 |  | 1.002 [1.001, 1.002] | 1.000 [0.982, 1.020] | 2453.396 / 2473.984 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | prime_dot | q100_n1048576 | acc4 | yes | 0.990 [0.989, 0.991] | 0.990 [0.969, 1.000] | 2424.917 / 2460.300 | 32.000 | 0 / 0 | pass |
| aarch64 | prime_dot | q100_n1048576 | acc8 |  | 1.031 [1.030, 1.032] | 1.029 [1.010, 1.042] | 2526.604 / 2564.537 | 32.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1048576 | checked_per_term |  | 1.232 [1.227, 1.234] | 1.229 [1.205, 1.237] | 3019.958 / 3042.761 | 32.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1048576 | eager_raw |  | 2.751 [2.748, 2.753] | 2.748 [2.707, 2.808] | 6736.833 / 6840.177 | 32.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1048576 | crypto_bigint_reduce |  | 1.000 [0.999, 1.001] | 0.998 [0.981, 1.006] | 2451.260 / 2474.592 | 32.000 | 0 / 0 | pass |
| aarch64 | prime_dot | q100_n1048576 | convert_then_delayed |  | 1.261 [1.260, 1.271] | 1.270 [1.240, 1.294] | 3095.698 / 3158.656 | 64.000 | 2 / 33554432 | regression |
| aarch64 | prime_dot | q128_n16 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.058 / 0.058 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_dot | q128_n16 | acc2 |  | 1.014 [1.013, 1.015] | 1.016 [1.000, 1.028] | 0.059 / 0.059 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n16 | acc4 | yes | 1.044 [1.044, 1.046] | 1.051 [1.035, 1.067] | 0.061 / 0.062 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n16 | acc8 |  | 1.095 [1.094, 1.097] | 1.094 [1.075, 1.101] | 0.063 / 0.064 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n16 | checked_per_term |  | 1.155 [1.152, 1.156] | 1.161 [1.136, 1.175] | 0.067 / 0.067 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n16 | eager_raw |  | 1.745 [1.744, 1.810] | 1.766 [1.741, 1.810] | 0.101 / 0.105 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n16 | crypto_bigint_reduce |  | 2.576 [2.575, 2.618] | 2.593 [2.562, 2.653] | 0.150 / 0.152 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n16 | convert_then_delayed |  | 2.043 [1.992, 2.060] | 2.046 [1.979, 2.083] | 0.119 / 0.120 | 0.001 | 2 / 512 | regression |
| aarch64 | prime_dot | q128_n1024 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.390 / 2.474 | 0.031 | 0 / 0 | pass |
| aarch64 | prime_dot | q128_n1024 | acc2 |  | 0.997 [0.996, 0.997] | 0.995 [0.973, 1.022] | 2.382 / 2.470 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | prime_dot | q128_n1024 | acc4 | yes | 0.988 [0.986, 0.989] | 0.988 [0.961, 1.010] | 2.362 / 2.441 | 0.031 | 0 / 0 | pass |
| aarch64 | prime_dot | q128_n1024 | acc8 |  | 1.040 [1.039, 1.041] | 1.039 [1.006, 1.055] | 2.485 / 2.578 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n1024 | checked_per_term |  | 1.219 [1.217, 1.223] | 1.219 [1.173, 1.243] | 2.916 / 3.002 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n1024 | eager_raw |  | 2.749 [2.748, 2.751] | 2.709 [2.642, 2.768] | 6.575 / 6.727 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n1024 | crypto_bigint_reduce |  | 1.049 [1.046, 1.050] | 1.044 [1.010, 1.058] | 2.507 / 2.578 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n1024 | convert_then_delayed |  | 1.212 [1.208, 1.215] | 1.195 [1.172, 1.240] | 2.895 / 2.995 | 0.062 | 2 / 32768 | regression |
| aarch64 | prime_dot | q128_n1048576 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2454.104 / 2469.870 | 32.000 | 0 / 0 | pass |
| aarch64 | prime_dot | q128_n1048576 | acc2 |  | 1.001 [1.001, 1.002] | 1.002 [1.000, 1.006] | 2455.448 / 2472.095 | 32.000 | 0 / 0 | pass |
| aarch64 | prime_dot | q128_n1048576 | acc4 | yes | 0.989 [0.989, 0.991] | 0.990 [0.988, 1.001] | 2426.865 / 2444.218 | 32.000 | 0 / 0 | pass |
| aarch64 | prime_dot | q128_n1048576 | acc8 |  | 1.031 [1.030, 1.033] | 1.032 [1.030, 1.055] | 2529.760 / 2558.460 | 32.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n1048576 | checked_per_term |  | 1.229 [1.228, 1.234] | 1.232 [1.225, 1.237] | 3018.406 / 3039.284 | 32.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n1048576 | eager_raw |  | 2.752 [2.749, 2.753] | 2.776 [2.745, 2.808] | 6752.771 / 6871.664 | 32.000 | 0 / 0 | regression |
| aarch64 | prime_dot | q128_n1048576 | crypto_bigint_reduce |  | 1.000 [0.999, 1.001] | 1.011 [0.998, 1.030] | 2453.969 / 2489.661 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | prime_dot | q128_n1048576 | convert_then_delayed |  | 1.268 [1.259, 1.270] | 1.272 [1.264, 1.300] | 3103.709 / 3170.487 | 64.000 | 2 / 33554432 | regression |
| aarch64 | prime_linear | q100_n16 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.040 / 0.040 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n16 | acc2 |  | 1.002 [1.001, 1.002] | 1.000 [0.994, 1.008] | 0.040 / 0.040 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n16 | acc4 | yes | 0.996 [0.995, 0.998] | 0.996 [0.980, 1.005] | 0.040 / 0.040 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n16 | acc8 |  | 1.064 [1.063, 1.065] | 1.064 [1.040, 1.075] | 0.042 / 0.043 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_linear | q100_n16 | crypto_bigint_reduce |  | 3.272 [3.259, 3.311] | 3.262 [3.237, 3.344] | 0.130 / 0.133 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_linear | q100_n1024 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.496 / 1.546 | 0.023 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n1024 | acc2 |  | 0.944 [0.940, 0.947] | 0.937 [0.928, 0.948] | 1.412 / 1.445 | 0.023 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n1024 | acc4 | yes | 0.885 [0.884, 0.887] | 0.885 [0.880, 0.903] | 1.324 / 1.365 | 0.023 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n1024 | acc8 |  | 0.881 [0.878, 0.882] | 0.894 [0.876, 0.908] | 1.316 / 1.364 | 0.023 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n1024 | crypto_bigint_reduce |  | 1.059 [1.055, 1.064] | 1.057 [1.049, 1.074] | 1.581 / 1.625 | 0.023 | 0 / 0 | regression |
| aarch64 | prime_linear | q100_n1048576 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1515.354 / 1545.614 | 24.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n1048576 | acc2 |  | 0.943 [0.935, 0.945] | 0.943 [0.923, 0.955] | 1425.156 / 1460.917 | 24.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n1048576 | acc4 | yes | 0.887 [0.882, 0.888] | 0.890 [0.871, 0.910] | 1341.552 / 1381.911 | 24.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n1048576 | acc8 |  | 0.881 [0.880, 0.882] | 0.873 [0.856, 0.882] | 1334.125 / 1350.167 | 24.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q100_n1048576 | crypto_bigint_reduce |  | 1.000 [0.999, 1.001] | 1.000 [0.986, 1.025] | 1514.875 / 1535.812 | 24.000 | 0 / 0 | inconclusive |
| aarch64 | prime_linear | q128_n16 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.040 / 0.040 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q128_n16 | acc2 |  | 1.001 [1.001, 1.002] | 1.002 [0.997, 1.013] | 0.040 / 0.041 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | prime_linear | q128_n16 | acc4 | yes | 0.996 [0.995, 0.998] | 0.994 [0.983, 1.002] | 0.040 / 0.040 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q128_n16 | acc8 |  | 1.064 [1.063, 1.066] | 1.063 [1.050, 1.071] | 0.042 / 0.043 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_linear | q128_n16 | crypto_bigint_reduce |  | 3.221 [3.216, 3.233] | 3.218 [3.184, 3.247] | 0.128 / 0.130 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_linear | q128_n1024 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.487 / 1.500 | 0.023 | 0 / 0 | pass |
| aarch64 | prime_linear | q128_n1024 | acc2 |  | 0.948 [0.945, 0.954] | 0.947 [0.941, 0.954] | 1.414 / 1.420 | 0.023 | 0 / 0 | pass |
| aarch64 | prime_linear | q128_n1024 | acc4 | yes | 0.889 [0.889, 0.893] | 0.892 [0.885, 0.898] | 1.325 / 1.334 | 0.023 | 0 / 0 | pass |
| aarch64 | prime_linear | q128_n1024 | acc8 |  | 0.885 [0.884, 0.886] | 0.885 [0.881, 0.894] | 1.317 / 1.324 | 0.023 | 0 / 0 | pass |
| aarch64 | prime_linear | q128_n1024 | crypto_bigint_reduce |  | 1.064 [1.063, 1.065] | 1.064 [1.059, 1.068] | 1.582 / 1.596 | 0.023 | 0 / 0 | regression |
| aarch64 | prime_linear | q128_n1048576 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1514.698 / 1525.786 | 24.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q128_n1048576 | acc2 |  | 0.937 [0.937, 0.944] | 0.940 [0.925, 0.948] | 1423.989 / 1436.117 | 24.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q128_n1048576 | acc4 | yes | 0.885 [0.884, 0.888] | 0.889 [0.871, 0.906] | 1341.417 / 1358.345 | 24.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q128_n1048576 | acc8 |  | 0.881 [0.880, 0.882] | 0.881 [0.878, 0.899] | 1333.864 / 1363.890 | 24.000 | 0 / 0 | pass |
| aarch64 | prime_linear | q128_n1048576 | crypto_bigint_reduce |  | 1.000 [1.000, 1.002] | 1.007 [0.992, 1.023] | 1516.386 / 1547.530 | 24.000 | 0 / 0 | inconclusive |
| aarch64 | prime_reduce_product | 100 | existing_optimized |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.016 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_reduce_product | 100 | crypto_bigint |  | 6.680 [6.670, 8.782] | 6.678 [6.521, 8.613] | 0.106 / 0.140 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_reduce_product | 128 | existing_optimized |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.016 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_reduce_product | 128 | crypto_bigint |  | 6.609 [6.604, 8.725] | 6.573 [6.471, 8.738] | 0.105 / 0.139 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_reduce_linear | 100 | existing_optimized |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_reduce_linear | 100 | crypto_bigint |  | 7.514 [7.507, 7.531] | 7.515 [7.379, 7.546] | 0.100 / 0.105 | 0.000 | 0 / 0 | regression |
| aarch64 | prime_reduce_linear | 128 | existing_optimized |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.013 / 0.014 | 0.000 | 0 / 0 | pass |
| aarch64 | prime_reduce_linear | 128 | crypto_bigint |  | 7.435 [7.429, 7.450] | 7.410 [7.259, 7.468] | 0.098 / 0.100 | 0.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l1_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.005 / 0.006 | 0.000 | 0 / 0 | pass |
| aarch64 | integer_mac | l1_n16 | fused | yes | 1.057 [1.052, 1.070] | 1.070 [1.048, 1.078] | 0.006 / 0.006 | 0.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l1_n16 | checked_per_term |  | 2.616 [2.410, 2.647] | 2.601 [2.419, 2.674] | 0.014 / 0.015 | 0.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l1_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.322 / 0.325 | 0.016 | 0 / 0 | pass |
| aarch64 | integer_mac | l1_n1024 | fused | yes | 1.003 [1.003, 1.046] | 1.003 [0.997, 1.037] | 0.323 / 0.339 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | integer_mac | l1_n1024 | checked_per_term |  | 2.383 [2.382, 2.385] | 2.382 [2.351, 2.420] | 0.768 / 0.778 | 0.016 | 0 / 0 | regression |
| aarch64 | integer_mac | l1_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 20.408 / 20.563 | 1.000 | 0 / 0 | pass |
| aarch64 | integer_mac | l1_n65536 | fused | yes | 1.000 [1.000, 1.001] | 1.002 [0.992, 1.015] | 20.419 / 20.582 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | integer_mac | l1_n65536 | checked_per_term |  | 2.374 [2.373, 2.376] | 2.380 [2.345, 2.399] | 48.463 / 48.992 | 1.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l2_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.012 / 0.013 | 0.000 | 0 / 0 | pass |
| aarch64 | integer_mac | l2_n16 | fused | yes | 1.298 [1.265, 1.560] | 1.304 [1.263, 1.563] | 0.016 / 0.019 | 0.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l2_n16 | checked_per_term |  | 1.724 [1.709, 1.770] | 1.754 [1.712, 1.787] | 0.022 / 0.022 | 0.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l2_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.675 / 0.687 | 0.031 | 0 / 0 | pass |
| aarch64 | integer_mac | l2_n1024 | fused | yes | 2.344 [2.337, 2.381] | 2.349 [2.309, 2.375] | 1.581 / 1.616 | 0.031 | 0 / 0 | regression |
| aarch64 | integer_mac | l2_n1024 | checked_per_term |  | 2.354 [2.351, 2.356] | 2.358 [2.325, 2.391] | 1.590 / 1.627 | 0.031 | 0 / 0 | regression |
| aarch64 | integer_mac | l2_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 43.074 / 44.053 | 2.000 | 0 / 0 | pass |
| aarch64 | integer_mac | l2_n65536 | fused | yes | 2.367 [2.361, 2.375] | 2.355 [2.346, 2.387] | 101.938 / 103.806 | 2.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l2_n65536 | checked_per_term |  | 2.375 [2.372, 2.380] | 2.368 [2.350, 2.405] | 102.337 / 104.441 | 2.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l4_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.052 / 0.053 | 0.001 | 0 / 0 | pass |
| aarch64 | integer_mac | l4_n16 | fused | yes | 1.087 [1.084, 1.089] | 1.089 [1.085, 1.099] | 0.057 / 0.057 | 0.001 | 0 / 0 | regression |
| aarch64 | integer_mac | l4_n16 | checked_per_term |  | 1.091 [1.089, 1.097] | 1.092 [1.088, 1.104] | 0.057 / 0.058 | 0.001 | 0 / 0 | regression |
| aarch64 | integer_mac | l4_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.112 / 3.169 | 0.062 | 0 / 0 | pass |
| aarch64 | integer_mac | l4_n1024 | fused | yes | 1.328 [1.325, 1.330] | 1.324 [1.306, 1.330] | 4.130 / 4.190 | 0.062 | 0 / 0 | regression |
| aarch64 | integer_mac | l4_n1024 | checked_per_term |  | 1.138 [1.135, 1.183] | 1.136 [1.123, 1.182] | 3.534 / 3.733 | 0.062 | 0 / 0 | regression |
| aarch64 | integer_mac | l4_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 202.307 / 204.228 | 4.000 | 0 / 0 | pass |
| aarch64 | integer_mac | l4_n65536 | fused | yes | 1.310 [1.307, 1.312] | 1.310 [1.305, 1.313] | 265.016 / 266.709 | 4.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l4_n65536 | checked_per_term |  | 1.135 [1.130, 1.136] | 1.133 [1.127, 1.136] | 229.367 / 231.154 | 4.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l9_n16 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.327 / 0.330 | 0.002 | 0 / 0 | pass |
| aarch64 | integer_mac | l9_n16 | fused | yes | 0.824 [0.824, 0.826] | 0.825 [0.823, 0.835] | 0.270 / 0.272 | 0.002 | 0 / 0 | pass |
| aarch64 | integer_mac | l9_n16 | checked_per_term |  | 0.967 [0.966, 0.971] | 0.973 [0.965, 0.993] | 0.317 / 0.320 | 0.002 | 0 / 0 | pass |
| aarch64 | integer_mac | l9_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.263 / 21.451 | 0.141 | 0 / 0 | pass |
| aarch64 | integer_mac | l9_n1024 | fused | yes | 0.810 [0.808, 0.810] | 0.806 [0.801, 0.811] | 17.197 / 17.295 | 0.141 | 0 / 0 | pass |
| aarch64 | integer_mac | l9_n1024 | checked_per_term |  | 0.953 [0.950, 0.954] | 0.948 [0.947, 0.957] | 20.248 / 20.354 | 0.141 | 0 / 0 | pass |
| aarch64 | integer_mac | l9_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1376.292 / 1430.304 | 9.000 | 0 / 0 | pass |
| aarch64 | integer_mac | l9_n65536 | fused | yes | 0.811 [0.810, 0.813] | 0.815 [0.803, 0.822] | 1118.286 / 1165.997 | 9.000 | 0 / 0 | pass |
| aarch64 | integer_mac | l9_n65536 | checked_per_term |  | 0.949 [0.946, 0.953] | 0.945 [0.935, 0.954] | 1305.818 / 1350.482 | 9.000 | 0 / 0 | pass |
| aarch64 | p256_product | active4_n16 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.325 / 0.328 | 0.004 | 0 / 0 | pass |
| aarch64 | p256_product | active4_n16 | fixed9 |  | 3.030 [2.902, 3.081] | 3.137 [3.048, 3.195] | 0.977 / 1.034 | 0.004 | 0 / 0 | regression |
| aarch64 | p256_product | active4_n16 | public_bound4 | yes | 0.315 [0.315, 0.316] | 0.315 [0.313, 0.316] | 0.102 / 0.103 | 0.004 | 0 / 0 | pass |
| aarch64 | p256_product | active4_n1024 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 21.045 / 22.111 | 0.281 | 0 / 0 | pass |
| aarch64 | p256_product | active4_n1024 | fixed9 |  | 3.075 [2.936, 3.102] | 3.108 [3.016, 3.160] | 64.554 / 67.754 | 0.281 | 0 / 0 | regression |
| aarch64 | p256_product | active4_n1024 | public_bound4 | yes | 0.315 [0.314, 0.316] | 0.315 [0.306, 0.317] | 6.625 / 6.842 | 0.281 | 0 / 0 | pass |
| aarch64 | p256_product | active4_n65536 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1374.036 / 1431.608 | 18.000 | 0 / 0 | pass |
| aarch64 | p256_product | active4_n65536 | fixed9 |  | 3.126 [3.086, 3.145] | 3.122 [3.059, 3.145] | 4300.005 / 4418.657 | 18.000 | 0 / 0 | regression |
| aarch64 | p256_product | active4_n65536 | public_bound4 | yes | 0.326 [0.326, 0.328] | 0.328 [0.322, 0.333] | 449.167 / 464.908 | 18.000 | 0 / 0 | pass |
| aarch64 | p256_product | active9_n16 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.921 / 0.939 | 0.004 | 0 / 0 | pass |
| aarch64 | p256_product | active9_n16 | fixed9 | yes | 1.038 [1.006, 1.073] | 1.087 [1.053, 1.110] | 0.942 / 1.030 | 0.004 | 0 / 0 | regression |
| aarch64 | p256_product | active9_n1024 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 58.942 / 59.931 | 0.281 | 0 / 0 | pass |
| aarch64 | p256_product | active9_n1024 | fixed9 | yes | 1.079 [1.057, 1.095] | 1.121 [1.099, 1.137] | 63.166 / 67.177 | 0.281 | 0 / 0 | regression |
| aarch64 | p256_product | active9_n65536 | existing_p256 |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3799.938 / 3919.035 | 18.000 | 0 / 0 | pass |
| aarch64 | p256_product | active9_n65536 | fixed9 | yes | 1.111 [1.076, 1.118] | 1.118 [1.079, 1.143] | 4212.812 / 4366.367 | 18.000 | 0 / 0 | regression |
| aarch64 | projection | q100_l2_n16 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.167 / 3.204 | 0.001 | 9 / 144 | pass |
| aarch64 | projection | q100_l2_n16 | fixed_crypto_bigint_vartime |  | 0.245 [0.244, 0.248] | 0.245 [0.243, 0.248] | 0.778 / 0.784 | 0.000 | 0 / 0 | pass |
| aarch64 | projection | q100_l2_n1024 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 547.643 / 552.829 | 0.047 | 522 / 8352 | pass |
| aarch64 | projection | q100_l2_n1024 | fixed_crypto_bigint_vartime |  | 0.090 [0.090, 0.090] | 0.090 [0.088, 0.090] | 49.135 / 49.616 | 0.031 | 0 / 0 | pass |
| aarch64 | projection | q100_l4_n16 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 6.479 / 7.435 | 0.001 | 10 / 320 | pass |
| aarch64 | projection | q100_l4_n16 | fixed_crypto_bigint_vartime |  | 0.190 [0.175, 0.192] | 0.180 [0.152, 0.190] | 1.231 / 1.246 | 0.001 | 0 / 0 | pass |
| aarch64 | projection | q100_l4_n1024 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1333.448 / 1352.483 | 0.062 | 532 / 17024 | pass |
| aarch64 | projection | q100_l4_n1024 | fixed_crypto_bigint_vartime |  | 0.059 [0.058, 0.059] | 0.059 [0.058, 0.059] | 78.182 / 79.466 | 0.047 | 0 / 0 | pass |
| aarch64 | projection | q100_l9_n16 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 32.361 / 41.140 | 0.002 | 12 / 864 | pass |
| aarch64 | projection | q100_l9_n16 | fixed_crypto_bigint_vartime |  | 0.077 [0.064, 0.079] | 0.070 [0.058, 0.074] | 2.409 / 2.496 | 0.001 | 0 / 0 | pass |
| aarch64 | projection | q100_l9_n1024 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3296.688 / 3419.890 | 0.102 | 536 / 38592 | pass |
| aarch64 | projection | q100_l9_n1024 | fixed_crypto_bigint_vartime |  | 0.047 [0.047, 0.047] | 0.047 [0.046, 0.047] | 154.250 / 158.918 | 0.086 | 0 / 0 | pass |
| aarch64 | projection | q128_l2_n16 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.338 / 0.416 | 0.001 | 0 / 0 | pass |
| aarch64 | projection | q128_l2_n16 | fixed_crypto_bigint_vartime |  | 2.163 [1.795, 2.355] | 2.160 [1.788, 2.299] | 0.738 / 0.742 | 0.000 | 0 / 0 | regression |
| aarch64 | projection | q128_l2_n1024 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 22.809 / 23.330 | 0.047 | 0 / 0 | pass |
| aarch64 | projection | q128_l2_n1024 | fixed_crypto_bigint_vartime |  | 2.021 [2.010, 2.079] | 2.040 [2.010, 2.067] | 46.437 / 47.325 | 0.031 | 0 / 0 | regression |
| aarch64 | projection | q128_l4_n16 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.216 / 1.343 | 0.001 | 0 / 0 | pass |
| aarch64 | projection | q128_l4_n16 | fixed_crypto_bigint_vartime |  | 0.947 [0.888, 1.063] | 0.922 [0.865, 0.963] | 1.162 / 1.173 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | projection | q128_l4_n1024 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 84.993 / 88.625 | 0.062 | 0 / 0 | pass |
| aarch64 | projection | q128_l4_n1024 | fixed_crypto_bigint_vartime |  | 0.868 [0.850, 0.908] | 0.852 [0.838, 0.873] | 73.942 / 75.940 | 0.047 | 0 / 0 | pass |
| aarch64 | projection | q128_l9_n16 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 3.419 / 3.553 | 0.002 | 0 / 0 | pass |
| aarch64 | projection | q128_l9_n16 | fixed_crypto_bigint_vartime |  | 0.685 [0.665, 0.742] | 0.669 [0.661, 0.741] | 2.342 / 2.360 | 0.001 | 0 / 0 | pass |
| aarch64 | projection | q128_l9_n1024 | runtime_modulus |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 231.477 / 236.123 | 0.102 | 0 / 0 | pass |
| aarch64 | projection | q128_l9_n1024 | fixed_crypto_bigint_vartime |  | 0.648 [0.642, 0.653] | 0.647 [0.624, 0.655] | 149.957 / 151.135 | 0.086 | 0 / 0 | pass |
