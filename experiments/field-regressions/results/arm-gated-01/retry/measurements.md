# GF128 and NTT regression measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | dot | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 16 | wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | dot | 16 | wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 16 | wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 16 | wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 16 | wide16 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 16 | vec2 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1024 | wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1024 | wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1024 | wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1024 | wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1024 | wide16 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1024 | vec2 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 65536 | wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | dot | 65536 | wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 65536 | wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 65536 | wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 65536 | wide16 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 65536 | vec2 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | wide16 |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | vec2 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | products | 16 | shared |  | 1.019 [1.015, 1.024] | 1.019 [0.971, 1.045] | 0.017 / 0.018 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | shared_unroll2 |  | 1.052 [1.047, 1.056] | 1.057 [1.010, 1.125] | 0.017 / 0.019 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | shared_unroll4 |  | 1.067 [1.064, 1.073] | 1.067 [1.013, 1.096] | 0.017 / 0.019 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | shared_unroll8 |  | 1.077 [1.073, 1.084] | 1.073 [1.030, 1.269] | 0.018 / 0.019 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | scalar_lanes | yes | 0.999 [0.993, 1.002] | 0.997 [0.936, 1.036] | 0.016 / 0.017 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | products | 16 | schoolbook |  | 2.668 [2.657, 2.679] | 2.673 [2.519, 2.735] | 0.044 / 0.047 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | karatsuba |  | 3.131 [3.113, 3.146] | 3.102 [2.938, 3.232] | 0.051 / 0.054 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | karatsuba_barrett |  | 1.745 [1.731, 1.764] | 1.753 [1.697, 1.849] | 0.029 / 0.032 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared_unroll2 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared_unroll4 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared_unroll8 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 55.071 / 58.298 | 3.000 | 0 / 0 | pass |
| aarch64 | products | 65536 | shared |  | 1.068 [1.063, 1.072] | 1.065 [1.036, 1.103] | 58.798 / 61.972 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | shared_unroll2 |  | 1.068 [1.063, 1.072] | 1.068 [0.985, 1.077] | 58.831 / 61.505 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | shared_unroll4 |  | 1.068 [1.063, 1.073] | 1.069 [1.037, 1.089] | 58.884 / 62.159 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | shared_unroll8 |  | 1.068 [1.063, 1.073] | 1.068 [1.057, 1.108] | 58.909 / 63.566 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | scalar_lanes | yes | 1.002 [0.996, 1.006] | 1.003 [0.951, 1.017] | 55.245 / 58.280 | 3.000 | 0 / 0 | inconclusive |
| aarch64 | products | 65536 | schoolbook |  | 3.062 [3.049, 3.076] | 3.050 [2.939, 3.111] | 168.935 / 178.874 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | karatsuba |  | 3.581 [3.574, 3.600] | 3.570 [3.505, 3.637] | 197.797 / 207.929 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 65536 | karatsuba_barrett |  | 1.912 [1.904, 1.917] | 1.900 [1.874, 1.957] | 105.285 / 110.525 | 3.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 901.354 / 1109.541 | 48.000 | 0 / 0 | pass |
| aarch64 | products | 1048576 | shared |  | 1.062 [1.055, 1.067] | 1.065 [0.934, 1.119] | 955.281 / 1113.179 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | shared_unroll2 |  | 1.055 [1.051, 1.061] | 1.051 [0.979, 1.151] | 953.427 / 1144.366 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | shared_unroll4 |  | 1.059 [1.053, 1.066] | 1.033 [0.923, 1.109] | 955.906 / 1124.356 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | shared_unroll8 |  | 1.061 [1.054, 1.070] | 1.045 [0.946, 1.111] | 957.214 / 1102.360 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | scalar_lanes | yes | 0.997 [0.992, 1.002] | 1.006 [0.937, 1.069] | 900.662 / 1123.664 | 48.000 | 0 / 0 | inconclusive |
| aarch64 | products | 1048576 | schoolbook |  | 3.001 [2.988, 3.018] | 2.794 [2.575, 3.005] | 2707.896 / 3117.535 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | karatsuba |  | 3.514 [3.498, 3.539] | 3.213 [3.035, 3.543] | 3178.688 / 3675.767 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | karatsuba_barrett |  | 1.900 [1.888, 1.907] | 1.747 [1.635, 1.959] | 1713.995 / 1987.534 | 48.000 | 0 / 0 | regression |
| aarch64 | chain | 16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.061 / 0.063 | 0.000 | 0 / 0 | pass |
| aarch64 | chain | 16 | shared |  | 0.993 [0.988, 1.012] | 0.998 [0.987, 1.021] | 0.060 / 0.063 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | chain | 16 | scalar_lanes | yes | 0.989 [0.985, 0.998] | 0.998 [0.989, 1.012] | 0.060 / 0.063 | 0.000 | 0 / 0 | inconclusive |
| aarch64 | chain | 16 | schoolbook |  | 1.874 [1.869, 1.890] | 1.873 [1.856, 1.896] | 0.114 / 0.117 | 0.000 | 0 / 0 | regression |
| aarch64 | chain | 16 | karatsuba |  | 1.859 [1.852, 1.872] | 1.857 [1.824, 1.884] | 0.113 / 0.116 | 0.000 | 0 / 0 | regression |
| aarch64 | chain | 16 | karatsuba_barrett |  | 1.366 [1.360, 1.379] | 1.367 [1.353, 1.388] | 0.083 / 0.086 | 0.000 | 0 / 0 | regression |
| aarch64 | chain | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 521.086 / 541.188 | 1.000 | 0 / 0 | pass |
| aarch64 | chain | 65536 | shared |  | 0.986 [0.984, 0.991] | 0.985 [0.977, 0.995] | 514.156 / 533.009 | 1.000 | 0 / 0 | pass |
| aarch64 | chain | 65536 | scalar_lanes | yes | 0.988 [0.985, 0.991] | 0.983 [0.974, 0.994] | 514.469 / 533.503 | 1.000 | 0 / 0 | pass |
| aarch64 | chain | 65536 | schoolbook |  | 1.247 [1.244, 1.253] | 1.242 [1.229, 1.264] | 650.508 / 674.935 | 1.000 | 0 / 0 | regression |
| aarch64 | chain | 65536 | karatsuba |  | 1.095 [1.087, 1.102] | 1.092 [1.080, 1.105] | 569.667 / 592.261 | 1.000 | 0 / 0 | regression |
| aarch64 | chain | 65536 | karatsuba_barrett |  | 1.231 [1.224, 1.239] | 1.234 [1.217, 1.244] | 642.359 / 668.521 | 1.000 | 0 / 0 | regression |
| aarch64 | chain | 1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | square | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | square | 16 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | square | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | square | 1024 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | square | 65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | square | 65536 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | square | 1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | square | 1048576 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 1 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 1 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 16 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 1024 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed_prepare | zero | existing_prepare |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | fixed_prepare | half | existing_prepare |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | fixed_prepare | full | existing_prepare |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | fixed | zero_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | specialized |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.837 / 1.069 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | zero_n1024 | shared |  | 1.049 [1.040, 1.057] | 1.035 [0.990, 1.205] | 0.875 / 1.046 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | zero_n1024 | prepared |  | 0.898 [0.894, 0.909] | 0.878 [0.779, 0.907] | 0.751 / 0.898 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | zero_n1024 | specialized |  | 0.217 [0.213, 0.231] | 0.235 [0.203, 0.297] | 0.181 / 0.249 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | zero_n1024 | scalar_lanes | yes | 1.002 [0.994, 1.010] | 0.993 [0.909, 1.067] | 0.837 / 1.011 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | fixed | zero_n65536 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 52.898 / 75.746 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n65536 | shared |  | 1.042 [1.015, 1.042] | 1.020 [0.990, 1.045] | 54.788 / 77.402 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | zero_n65536 | prepared |  | 0.897 [0.896, 0.905] | 0.898 [0.864, 0.911] | 47.468 / 68.154 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n65536 | specialized |  | 0.212 [0.183, 0.279] | 0.330 [0.268, 0.386] | 11.181 / 21.457 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n65536 | scalar_lanes | yes | 1.000 [0.999, 1.003] | 0.996 [0.966, 1.010] | 53.010 / 75.801 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | zero_n1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 871.265 / 7217.541 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n1048576 | shared |  | 0.997 [0.988, 1.009] | 1.007 [0.830, 1.018] | 871.323 / 7437.016 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | zero_n1048576 | prepared |  | 0.902 [0.888, 0.911] | 0.901 [0.836, 1.039] | 784.932 / 6279.083 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | zero_n1048576 | specialized |  | 0.174 [0.095, 0.176] | 0.180 [0.169, 0.224] | 153.573 / 703.663 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | zero_n1048576 | scalar_lanes | yes | 0.997 [0.987, 1.004] | 0.994 [0.914, 1.714] | 870.047 / 7872.084 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | half_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | specialized |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.844 / 4.658 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | half_n1024 | shared |  | 1.046 [1.041, 1.071] | 1.043 [1.020, 1.320] | 0.883 / 5.260 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | half_n1024 | prepared |  | 0.900 [0.896, 0.912] | 0.898 [0.660, 1.150] | 0.760 / 3.686 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | fixed | half_n1024 | specialized |  | 1.108 [1.100, 1.231] | 1.107 [0.940, 1.318] | 0.932 / 5.647 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | half_n1024 | scalar_lanes | yes | 0.999 [0.990, 1.018] | 1.001 [0.755, 1.292] | 0.845 / 4.473 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | fixed | half_n65536 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 56.441 / 242.478 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | half_n65536 | shared |  | 1.012 [0.994, 1.042] | 0.994 [0.875, 1.059] | 57.513 / 264.363 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | half_n65536 | prepared |  | 0.897 [0.888, 0.910] | 0.903 [0.730, 0.951] | 50.897 / 195.608 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | half_n65536 | specialized |  | 1.038 [1.008, 1.081] | 1.040 [0.941, 1.116] | 59.545 / 259.154 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | half_n65536 | scalar_lanes | yes | 1.002 [0.990, 1.008] | 1.001 [0.890, 1.142] | 56.497 / 234.320 | 2.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | half_n1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 945.792 / 6042.479 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | half_n1048576 | shared |  | 0.997 [0.948, 1.075] | 1.006 [0.845, 1.109] | 946.047 / 5367.530 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | half_n1048576 | prepared |  | 0.904 [0.878, 0.954] | 0.911 [0.751, 1.097] | 860.724 / 6702.692 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | half_n1048576 | specialized |  | 1.030 [0.975, 1.102] | 1.109 [0.931, 1.956] | 960.583 / 11119.916 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | half_n1048576 | scalar_lanes | yes | 1.008 [0.970, 1.049] | 1.011 [0.833, 1.192] | 948.021 / 7880.769 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | full_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | specialized |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.842 / 4.033 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | full_n1024 | shared |  | 1.047 [1.038, 1.064] | 1.062 [1.029, 1.098] | 0.882 / 4.761 | 0.031 | 0 / 0 | regression |
| aarch64 | fixed | full_n1024 | prepared |  | 0.896 [0.890, 0.908] | 0.902 [0.886, 0.913] | 0.755 / 3.834 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | full_n1024 | specialized |  | 0.903 [0.889, 0.910] | 0.907 [0.890, 0.924] | 0.760 / 3.604 | 0.031 | 0 / 0 | pass |
| aarch64 | fixed | full_n1024 | scalar_lanes | yes | 1.003 [0.991, 1.010] | 1.015 [0.985, 1.022] | 0.844 / 4.316 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | fixed | full_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | specialized |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 865.630 / 6136.385 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | full_n1048576 | shared |  | 1.002 [0.996, 1.010] | 1.011 [0.951, 1.038] | 869.630 / 5232.559 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | fixed | full_n1048576 | prepared |  | 0.894 [0.891, 0.903] | 0.898 [0.857, 0.934] | 777.474 / 5175.367 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | full_n1048576 | specialized |  | 0.894 [0.891, 0.903] | 0.905 [0.807, 0.917] | 774.844 / 5452.192 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | full_n1048576 | scalar_lanes | yes | 0.999 [0.994, 1.007] | 1.032 [0.971, 1.110] | 867.724 / 7112.227 | 32.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.024 | 0.001 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n16 | shared |  | 0.997 [0.992, 1.001] | 0.999 [0.946, 1.137] | 0.021 / 0.024 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n16 | prepared |  | 0.988 [0.985, 0.993] | 0.997 [0.964, 1.177] | 0.021 / 0.025 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n16 | specialized |  | 0.596 [0.595, 0.604] | 0.603 [0.446, 0.608] | 0.013 / 0.014 | 0.001 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n16 | scalar_lanes | yes | 0.994 [0.988, 0.999] | 1.001 [0.968, 1.074] | 0.021 / 0.024 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.173 / 1.469 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n1024 | shared |  | 0.998 [0.983, 1.003] | 1.011 [0.975, 1.095] | 1.174 / 1.474 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n1024 | prepared |  | 0.955 [0.935, 0.961] | 0.956 [0.890, 0.972] | 1.121 / 1.376 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n1024 | specialized |  | 0.508 [0.496, 0.514] | 0.524 [0.497, 0.611] | 0.601 / 0.766 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n1024 | scalar_lanes | yes | 0.998 [0.990, 1.006] | 1.005 [0.951, 1.067] | 1.176 / 1.505 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n65536 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 74.971 / 690.007 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n65536 | shared |  | 1.000 [0.988, 1.002] | 1.018 [0.993, 1.062] | 74.821 / 574.735 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n65536 | prepared |  | 0.951 [0.945, 0.954] | 0.961 [0.948, 1.100] | 71.201 / 645.178 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n65536 | specialized |  | 0.510 [0.506, 0.514] | 0.524 [0.514, 0.755] | 38.326 / 217.144 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n65536 | scalar_lanes | yes | 1.000 [0.993, 1.003] | 1.009 [0.991, 1.080] | 74.906 / 592.056 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1255.016 / 9915.021 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n1048576 | shared |  | 0.999 [0.985, 1.019] | 0.936 [0.710, 1.026] | 1257.307 / 9903.822 | 64.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n1048576 | prepared |  | 0.960 [0.940, 0.973] | 0.973 [0.877, 1.165] | 1212.412 / 10844.685 | 64.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | zero_n1048576 | specialized |  | 0.908 [0.777, 0.913] | 0.862 [0.744, 0.926] | 1134.927 / 11337.998 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | zero_n1048576 | scalar_lanes | yes | 1.001 [0.990, 1.021] | 0.985 [0.841, 1.053] | 1254.542 / 11908.616 | 64.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.022 / 0.111 | 0.001 | 0 / 0 | pass |
| aarch64 | butterfly | half_n16 | shared |  | 1.001 [0.964, 1.009] | 1.054 [0.994, 1.300] | 0.021 / 0.132 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n16 | prepared |  | 0.990 [0.959, 0.998] | 0.993 [0.971, 1.284] | 0.021 / 0.115 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n16 | specialized |  | 1.142 [1.132, 1.199] | 1.146 [1.121, 1.352] | 0.025 / 0.168 | 0.001 | 0 / 0 | regression |
| aarch64 | butterfly | half_n16 | scalar_lanes | yes | 1.002 [0.979, 1.014] | 1.021 [0.990, 1.239] | 0.021 / 0.173 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.190 / 9.526 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | half_n1024 | shared |  | 0.999 [0.967, 1.007] | 1.001 [0.907, 1.411] | 1.188 / 8.469 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n1024 | prepared |  | 0.956 [0.934, 0.964] | 0.953 [0.863, 1.410] | 1.134 / 8.894 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n1024 | specialized |  | 1.007 [0.994, 1.038] | 1.002 [0.931, 1.096] | 1.197 / 11.335 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n1024 | scalar_lanes | yes | 1.004 [0.979, 1.011] | 1.001 [0.793, 1.024] | 1.189 / 9.176 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n65536 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 77.830 / 428.189 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | half_n65536 | shared |  | 1.002 [0.996, 1.040] | 0.995 [0.935, 1.148] | 78.026 / 580.511 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n65536 | prepared |  | 0.952 [0.950, 1.073] | 0.897 [0.830, 0.998] | 74.097 / 501.761 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n65536 | specialized |  | 1.037 [1.031, 1.082] | 1.013 [0.934, 1.087] | 80.456 / 497.899 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | half_n65536 | scalar_lanes | yes | 1.003 [0.997, 1.048] | 1.009 [0.825, 1.125] | 77.697 / 513.110 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1312.854 / 11849.385 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | half_n1048576 | shared |  | 1.010 [0.989, 1.069] | 1.018 [0.900, 1.250] | 1323.854 / 11203.838 | 64.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n1048576 | prepared |  | 0.974 [0.957, 1.001] | 1.012 [0.903, 1.228] | 1304.063 / 10846.225 | 64.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | half_n1048576 | specialized |  | 1.048 [1.024, 1.079] | 1.021 [0.897, 1.270] | 1389.343 / 12244.373 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | half_n1048576 | scalar_lanes | yes | 1.014 [0.996, 1.055] | 1.021 [0.924, 1.344] | 1336.562 / 11971.017 | 64.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.021 / 0.164 | 0.001 | 0 / 0 | pass |
| aarch64 | butterfly | full_n16 | shared |  | 1.000 [0.978, 1.008] | 1.002 [0.978, 1.285] | 0.022 / 0.188 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n16 | prepared |  | 0.992 [0.988, 1.025] | 1.003 [0.956, 1.314] | 0.021 / 0.208 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n16 | specialized |  | 1.017 [1.005, 1.026] | 1.060 [1.010, 1.248] | 0.022 / 0.241 | 0.001 | 0 / 0 | regression |
| aarch64 | butterfly | full_n16 | scalar_lanes | yes | 1.000 [0.978, 1.010] | 1.028 [0.985, 1.407] | 0.022 / 0.256 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.176 / 6.959 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | full_n1024 | shared |  | 1.001 [0.991, 1.007] | 1.011 [0.987, 1.071] | 1.173 / 7.747 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n1024 | prepared |  | 0.955 [0.948, 0.961] | 0.954 [0.854, 0.969] | 1.121 / 5.472 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | full_n1024 | specialized |  | 0.957 [0.949, 0.963] | 0.957 [0.939, 1.070] | 1.120 / 7.223 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n1024 | scalar_lanes | yes | 0.997 [0.992, 1.005] | 1.014 [0.985, 1.054] | 1.174 / 6.299 | 0.062 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n65536 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 75.194 / 532.968 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n65536 | shared |  | 1.004 [0.962, 1.006] | 1.007 [0.770, 1.013] | 75.292 / 427.035 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n65536 | prepared |  | 0.955 [0.941, 0.960] | 0.955 [0.843, 0.978] | 71.752 / 525.612 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n65536 | specialized |  | 0.954 [0.949, 0.961] | 0.951 [0.830, 0.970] | 71.649 / 581.330 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n65536 | scalar_lanes | yes | 1.004 [0.983, 1.009] | 0.998 [0.926, 1.040] | 75.369 / 536.068 | 4.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1239.672 / 14997.369 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n1048576 | shared |  | 1.000 [0.985, 1.010] | 0.992 [0.915, 1.070] | 1236.391 / 12042.061 | 64.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n1048576 | prepared |  | 0.955 [0.947, 0.965] | 0.926 [0.784, 0.986] | 1186.391 / 15485.617 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n1048576 | specialized |  | 0.954 [0.948, 0.969] | 0.911 [0.745, 0.987] | 1182.938 / 13443.708 | 64.000 | 0 / 0 | inconclusive |
| aarch64 | butterfly | full_n1048576 | scalar_lanes | yes | 1.002 [0.931, 1.006] | 0.960 [0.706, 1.073] | 1243.609 / 11525.921 | 64.000 | 0 / 0 | inconclusive |
| aarch64 | ntt | log8_lanes1 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | preserved_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | shared_generic |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | shared_twiddle |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | prepared_twiddle |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | reset_only |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | ntt | log8_lanes32 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes32 | preserved_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes32 | shared_generic |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes32 | shared_twiddle |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes32 | prepared_twiddle |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes32 | reset_only |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | ntt | log12_lanes8 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 406.552 / 2322.733 | 1.000 | 0 / 0 | pass |
| aarch64 | ntt | log12_lanes8 | preserved_schedule | yes | 0.991 [0.983, 1.001] | 0.998 [0.927, 1.022] | 404.602 / 1820.051 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | ntt | log12_lanes8 | shared_generic |  | 0.947 [0.944, 0.965] | 0.940 [0.833, 0.962] | 384.820 / 1949.857 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | ntt | log12_lanes8 | shared_twiddle |  | 0.971 [0.964, 0.985] | 0.964 [0.931, 0.989] | 396.594 / 1824.641 | 1.000 | 0 / 0 | pass |
| aarch64 | ntt | log12_lanes8 | prepared_twiddle |  | 0.935 [0.932, 0.938] | 0.938 [0.885, 0.963] | 380.211 / 1420.973 | 1.000 | 0 / 0 | pass |
| aarch64 | ntt | log12_lanes8 | reset_only |  | 0.021 [0.020, 0.026] | 0.023 [0.021, 0.029] | 8.490 / 36.689 | 1.000 | 0 / 0 | unmeasured (diagnostic) |
| aarch64 | ntt | log15_lanes32 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log15_lanes32 | preserved_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log15_lanes32 | shared_generic |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log15_lanes32 | shared_twiddle |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log15_lanes32 | prepared_twiddle |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log15_lanes32 | reset_only |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | ntt | log16_lanes32 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log16_lanes32 | preserved_schedule | yes | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log16_lanes32 | shared_generic |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log16_lanes32 | shared_twiddle |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log16_lanes32 | prepared_twiddle |  | — | — | — | — | — | unmeasured |
| aarch64 | ntt | log16_lanes32 | reset_only |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | ntt | log17_lanes32 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 50293.771 / 91813.392 | 128.000 | 1 / 1520 | pass |
| aarch64 | ntt | log17_lanes32 | preserved_schedule | yes | 1.000 [0.997, 1.005] | 0.968 [0.741, 1.067] | 50304.812 / 73449.289 | 128.000 | 1 / 1520 | inconclusive |
| aarch64 | ntt | log17_lanes32 | shared_generic |  | 1.125 [1.117, 1.159] | 1.025 [0.626, 1.168] | 56401.313 / 73430.127 | 128.000 | 0 / 0 | regression |
| aarch64 | ntt | log17_lanes32 | shared_twiddle |  | 1.152 [1.145, 1.170] | 1.076 [0.635, 1.242] | 57813.708 / 75772.772 | 128.000 | 0 / 0 | regression |
| aarch64 | ntt | log17_lanes32 | prepared_twiddle |  | 1.120 [1.117, 1.143] | 1.021 [0.660, 1.173] | 56398.479 / 76179.881 | 128.000 | 0 / 0 | regression |
| aarch64 | ntt | log17_lanes32 | reset_only |  | 0.029 [0.029, 0.030] | 0.031 [0.020, 0.034] | 1447.021 / 2171.417 | 128.000 | 0 / 0 | unmeasured (diagnostic) |
| aarch64 | ntt | log18_lanes32 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 105398.042 / 426688.791 | 256.000 | 1 / 1520 | pass |
| aarch64 | ntt | log18_lanes32 | preserved_schedule | yes | 0.997 [0.992, 1.007] | 1.013 [0.973, 1.111] | 105490.895 / 411530.853 | 256.000 | 1 / 1520 | inconclusive |
| aarch64 | ntt | log18_lanes32 | shared_generic |  | 1.133 [1.123, 1.162] | 1.126 [1.021, 1.194] | 119124.250 / 456927.802 | 256.000 | 0 / 0 | regression |
| aarch64 | ntt | log18_lanes32 | shared_twiddle |  | 1.161 [1.152, 1.201] | 1.185 [1.135, 1.283] | 122061.270 / 492355.900 | 256.000 | 0 / 0 | regression |
| aarch64 | ntt | log18_lanes32 | prepared_twiddle |  | 1.134 [1.119, 1.171] | 1.135 [1.097, 1.212] | 119024.083 / 476205.400 | 256.000 | 0 / 0 | regression |
| aarch64 | ntt | log18_lanes32 | reset_only |  | 0.027 [0.024, 0.028] | 0.030 [0.029, 0.034] | 2893.188 / 8700.727 | 256.000 | 0 / 0 | unmeasured (diagnostic) |
