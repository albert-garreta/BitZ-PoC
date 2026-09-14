# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | products | 16 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | products | 16 | shared |  | 1.020 [1.019, 1.021] | 1.020 [1.009, 1.027] | 0.016 / 0.017 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | shared_unroll2 |  | 1.053 [1.052, 1.054] | 1.053 [1.043, 1.065] | 0.017 / 0.018 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | shared_unroll4 |  | 1.069 [1.068, 2.245] | 1.067 [1.060, 2.249] | 0.017 / 0.037 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | shared_unroll8 |  | 1.078 [1.078, 1.080] | 1.074 [1.066, 1.083] | 0.017 / 0.018 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | scalar_lanes | yes | 1.000 [1.000, 1.002] | 0.999 [0.983, 1.008] | 0.016 / 0.017 | 0.001 | 0 / 0 | pass |
| aarch64 | products | 16 | schoolbook |  | 2.663 [2.662, 2.665] | 2.664 [2.630, 2.682] | 0.043 / 0.045 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | karatsuba |  | 3.130 [3.128, 3.134] | 3.122 [3.095, 3.146] | 0.051 / 0.052 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 16 | karatsuba_barrett |  | 1.729 [1.723, 1.752] | 1.732 [1.715, 1.747] | 0.028 / 0.029 | 0.001 | 0 / 0 | regression |
| aarch64 | products | 1024 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.859 / 0.870 | 0.047 | 0 / 0 | pass |
| aarch64 | products | 1024 | shared |  | 1.063 [1.062, 1.063] | 1.063 [1.057, 1.066] | 0.913 / 0.925 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | shared_unroll2 |  | 1.063 [1.062, 1.064] | 1.063 [1.058, 1.066] | 0.913 / 0.925 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | shared_unroll4 |  | 1.063 [1.060, 1.063] | 1.061 [1.055, 1.064] | 0.913 / 0.923 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | shared_unroll8 |  | 1.063 [1.061, 1.064] | 1.060 [1.053, 1.076] | 0.914 / 0.923 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | scalar_lanes | yes | 0.999 [0.999, 1.000] | 0.999 [0.996, 1.003] | 0.859 / 0.870 | 0.047 | 0 / 0 | pass |
| aarch64 | products | 1024 | schoolbook |  | 3.041 [3.036, 3.043] | 3.041 [3.022, 3.052] | 2.612 / 2.640 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | karatsuba |  | 3.556 [3.553, 3.561] | 3.566 [3.532, 3.585] | 3.053 / 3.101 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 1024 | karatsuba_barrett |  | 1.864 [1.864, 1.872] | 1.866 [1.858, 1.882] | 1.601 / 1.632 | 0.047 | 0 / 0 | regression |
| aarch64 | products | 65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared_unroll2 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared_unroll4 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared_unroll8 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1048576 | flock |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 893.896 / 936.751 | 48.000 | 0 / 0 | pass |
| aarch64 | products | 1048576 | shared |  | 1.062 [1.061, 1.063] | 1.060 [1.038, 1.080] | 948.177 / 996.883 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | shared_unroll2 |  | 1.061 [1.059, 1.062] | 1.056 [1.036, 1.090] | 947.453 / 995.049 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | shared_unroll4 |  | 1.060 [1.058, 1.062] | 1.072 [1.046, 1.093] | 948.005 / 1008.799 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | shared_unroll8 |  | 1.061 [1.060, 1.063] | 1.059 [1.035, 1.079] | 948.401 / 996.561 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | scalar_lanes | yes | 1.001 [0.999, 1.002] | 1.014 [0.980, 1.050] | 893.328 / 947.040 | 48.000 | 0 / 0 | inconclusive |
| aarch64 | products | 1048576 | schoolbook |  | 3.020 [3.017, 3.026] | 2.975 [2.917, 3.021] | 2695.453 / 2795.500 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | karatsuba |  | 3.535 [3.528, 3.541] | 3.491 [3.398, 3.523] | 3154.766 / 3268.016 | 48.000 | 0 / 0 | regression |
| aarch64 | products | 1048576 | karatsuba_barrett |  | 1.904 [1.901, 1.907] | 1.876 [1.823, 1.900] | 1699.104 / 1766.829 | 48.000 | 0 / 0 | regression |
| aarch64 | chain | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
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
| aarch64 | fixed | zero_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | flock |  | 1.112 [1.108, 1.113] | 1.099 [1.088, 1.110] | 52.700 / 54.378 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n65536 | shared |  | 1.143 [1.117, 1.145] | 1.134 [1.090, 1.144] | 54.132 / 55.203 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n65536 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 47.379 / 49.693 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | full_n65536 | specialized | yes | 0.999 [0.998, 1.002] | 0.995 [0.987, 1.002] | 47.333 / 49.409 | 2.000 | 0 / 0 | pass |
| aarch64 | fixed | full_n65536 | scalar_lanes |  | 1.113 [1.106, 1.113] | 1.104 [1.084, 1.113] | 52.704 / 54.329 | 2.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n1048576 | flock |  | 1.112 [1.109, 1.113] | 1.101 [1.092, 1.117] | 855.552 / 887.137 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n1048576 | shared |  | 1.113 [1.108, 1.114] | 1.098 [1.092, 1.118] | 855.792 / 886.600 | 32.000 | 0 / 0 | regression |
| aarch64 | fixed | full_n1048576 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 768.703 / 803.990 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | full_n1048576 | specialized | yes | 1.000 [0.996, 1.002] | 0.996 [0.982, 1.007] | 769.260 / 803.558 | 32.000 | 0 / 0 | pass |
| aarch64 | fixed | full_n1048576 | scalar_lanes |  | 1.111 [1.110, 1.115] | 1.099 [1.092, 1.122] | 855.448 / 885.806 | 32.000 | 0 / 0 | regression |
| aarch64 | butterfly | zero_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | flock |  | 1.049 [1.047, 1.049] | 1.051 [1.046, 1.060] | 1.157 / 1.172 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | full_n1024 | shared |  | 1.049 [1.048, 1.084] | 1.049 [1.045, 1.081] | 1.157 / 1.207 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | full_n1024 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1.103 / 1.118 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | full_n1024 | specialized | yes | 1.001 [1.000, 1.002] | 1.000 [0.997, 1.007] | 1.103 / 1.120 | 0.062 | 0 / 0 | pass |
| aarch64 | butterfly | full_n1024 | scalar_lanes |  | 1.049 [1.048, 1.049] | 1.048 [1.046, 1.064] | 1.157 / 1.173 | 0.062 | 0 / 0 | regression |
| aarch64 | butterfly | full_n65536 | flock |  | 1.050 [1.049, 1.051] | 1.057 [1.045, 1.070] | 74.198 / 77.127 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n65536 | shared |  | 1.050 [1.049, 1.052] | 1.052 [1.043, 1.060] | 74.209 / 77.101 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n65536 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 70.674 / 73.446 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n65536 | specialized | yes | 1.000 [0.999, 1.001] | 1.001 [0.990, 1.009] | 70.645 / 73.467 | 4.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n65536 | scalar_lanes |  | 1.051 [1.050, 1.053] | 1.064 [1.047, 1.073] | 74.206 / 77.361 | 4.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n1048576 | flock |  | 1.047 [1.038, 1.048] | 1.035 [1.029, 1.055] | 1200.016 / 1259.714 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n1048576 | shared |  | 1.047 [1.036, 1.048] | 1.040 [1.031, 1.067] | 1201.016 / 1263.109 | 64.000 | 0 / 0 | regression |
| aarch64 | butterfly | full_n1048576 | prepared |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 1147.021 / 1214.100 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n1048576 | specialized | yes | 1.001 [0.998, 1.003] | 0.997 [0.987, 1.009] | 1148.182 / 1208.346 | 64.000 | 0 / 0 | pass |
| aarch64 | butterfly | full_n1048576 | scalar_lanes |  | 1.046 [1.039, 1.048] | 1.038 [1.021, 1.076] | 1200.375 / 1261.799 | 64.000 | 0 / 0 | regression |
| aarch64 | gf_dot | 16 | f2z_wide |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | flock_eager |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | shared_wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | shared_wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | shared_wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | shared_wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | f2z_wide |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | flock_eager |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | shared_wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | shared_wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | shared_wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | shared_wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | f2z_wide |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | flock_eager |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | shared_wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | shared_wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | shared_wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | shared_wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | f2z_wide |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | flock_eager |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | shared_wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | shared_wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | shared_wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | shared_wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_setup | 100 | existing_contexts |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | prime_setup | 128 | existing_contexts |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | prime_mul | q100_n16 | raw_ctx |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.077 / 0.080 | 0.001 | 0 / 0 | pass |
| aarch64 | prime_mul | q100_n16 | branded | yes | 1.008 [1.000, 1.010] | 1.000 [0.989, 1.025] | 0.078 / 0.081 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | prime_mul | q100_n16 | checked_per_term |  | 1.124 [1.104, 1.125] | 1.120 [1.098, 1.139] | 0.086 / 0.089 | 0.001 | 0 / 0 | regression |
| aarch64 | prime_mul | q100_n16 | configured_field |  | 1.600 [1.566, 1.602] | 1.589 [1.560, 1.618] | 0.123 / 0.126 | 0.004 | 0 / 0 | regression |
| aarch64 | prime_mul | q100_n1024 | raw_ctx |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.820 / 4.985 | 0.047 | 0 / 0 | pass |
| aarch64 | prime_mul | q100_n1024 | branded | yes | 1.000 [1.000, 1.001] | 0.999 [0.984, 1.012] | 4.823 / 4.992 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | prime_mul | q100_n1024 | checked_per_term |  | 1.116 [1.115, 1.116] | 1.112 [1.107, 1.120] | 5.377 / 5.546 | 0.047 | 0 / 0 | regression |
| aarch64 | prime_mul | q100_n1024 | configured_field |  | 1.605 [1.603, 1.608] | 1.593 [1.578, 1.617] | 7.735 / 7.950 | 0.234 | 0 / 0 | regression |
| aarch64 | prime_mul | q100_n1048576 | raw_ctx |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1048576 | branded | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1048576 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1048576 | configured_field |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n16 | raw_ctx |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.077 / 0.079 | 0.001 | 0 / 0 | pass |
| aarch64 | prime_mul | q128_n16 | branded | yes | 1.009 [0.988, 1.030] | 1.022 [0.989, 1.032] | 0.078 / 0.079 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | prime_mul | q128_n16 | checked_per_term |  | 1.124 [1.104, 1.126] | 1.129 [1.099, 1.143] | 0.087 / 0.087 | 0.001 | 0 / 0 | regression |
| aarch64 | prime_mul | q128_n16 | configured_field |  | 1.602 [1.566, 1.603] | 1.587 [1.566, 1.635] | 0.123 / 0.125 | 0.004 | 0 / 0 | regression |
| aarch64 | prime_mul | q128_n1024 | raw_ctx |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 4.827 / 4.909 | 0.047 | 0 / 0 | pass |
| aarch64 | prime_mul | q128_n1024 | branded | yes | 1.000 [0.999, 1.001] | 0.998 [0.974, 1.013] | 4.827 / 4.884 | 0.047 | 0 / 0 | inconclusive |
| aarch64 | prime_mul | q128_n1024 | checked_per_term |  | 1.115 [1.114, 1.116] | 1.121 [1.099, 1.128] | 5.383 / 5.452 | 0.047 | 0 / 0 | regression |
| aarch64 | prime_mul | q128_n1024 | configured_field |  | 1.605 [1.602, 1.607] | 1.604 [1.579, 1.629] | 7.747 / 7.904 | 0.234 | 0 / 0 | regression |
| aarch64 | prime_mul | q128_n1048576 | raw_ctx |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1048576 | branded | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1048576 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1048576 | configured_field |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1024 | existing_delayed |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 2.388 / 2.460 | 0.031 | 0 / 0 | pass |
| aarch64 | prime_dot | q100_n1024 | acc2 |  | 0.997 [0.996, 0.997] | 0.994 [0.974, 1.011] | 2.381 / 2.466 | 0.031 | 0 / 0 | inconclusive |
| aarch64 | prime_dot | q100_n1024 | acc4 | yes | 0.987 [0.987, 0.988] | 0.982 [0.970, 1.006] | 2.358 / 2.433 | 0.031 | 0 / 0 | pass |
| aarch64 | prime_dot | q100_n1024 | acc8 |  | 1.039 [1.038, 1.040] | 1.022 [1.011, 1.042] | 2.480 / 2.539 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1024 | checked_per_term |  | 1.222 [1.218, 1.223] | 1.209 [1.193, 1.240] | 2.915 / 3.002 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1024 | eager_raw |  | 2.749 [2.748, 2.751] | 2.705 [2.677, 2.769] | 6.565 / 6.706 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1024 | crypto_bigint_reduce |  | 1.043 [1.042, 1.043] | 1.031 [1.015, 1.050] | 2.491 / 2.554 | 0.031 | 0 / 0 | regression |
| aarch64 | prime_dot | q100_n1024 | convert_then_delayed |  | 1.209 [1.208, 1.210] | 1.199 [1.178, 1.217] | 2.887 / 2.957 | 0.062 | 2 / 32768 | regression |
| aarch64 | prime_dot | q100_n1048576 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n16 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n16 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n16 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n16 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n16 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1024 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1024 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1024 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1024 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1048576 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1048576 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1048576 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1048576 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1048576 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n16 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n16 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n16 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n16 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n16 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1024 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1024 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1024 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1024 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1048576 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1048576 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1048576 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1048576 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1048576 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_product | 100 | existing_optimized |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_product | 100 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_product | 128 | existing_optimized |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_product | 128 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_linear | 100 | existing_optimized |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_linear | 100 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_linear | 128 | existing_optimized |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_linear | 128 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n16 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n1024 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.323 / 0.342 | 0.016 | 0 / 0 | pass |
| aarch64 | integer_mac | l1_n1024 | fused | yes | 1.003 [0.999, 1.003] | 1.004 [0.997, 1.021] | 0.323 / 0.341 | 0.016 | 0 / 0 | inconclusive |
| aarch64 | integer_mac | l1_n1024 | checked_per_term |  | 2.384 [2.368, 2.386] | 2.383 [2.368, 2.418] | 0.769 / 0.811 | 0.016 | 0 / 0 | regression |
| aarch64 | integer_mac | l1_n65536 | circuit_z |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 20.414 / 21.897 | 1.000 | 0 / 0 | pass |
| aarch64 | integer_mac | l1_n65536 | fused | yes | 1.000 [0.999, 1.001] | 1.000 [0.978, 1.020] | 20.411 / 21.889 | 1.000 | 0 / 0 | inconclusive |
| aarch64 | integer_mac | l1_n65536 | checked_per_term |  | 2.374 [2.372, 2.376] | 2.373 [2.313, 2.407] | 48.462 / 51.940 | 1.000 | 0 / 0 | regression |
| aarch64 | integer_mac | l2_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n16 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n1024 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n65536 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n65536 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n16 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n1024 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n65536 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n65536 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n16 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n1024 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n65536 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n65536 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n16 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n16 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n16 | public_bound4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n1024 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n1024 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n1024 | public_bound4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n65536 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n65536 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n65536 | public_bound4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n16 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n16 | fixed9 | yes | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n1024 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n1024 | fixed9 | yes | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n65536 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n65536 | fixed9 | yes | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l2_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l2_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l2_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l2_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l4_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l4_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l4_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l4_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l9_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l9_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l9_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l9_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l2_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l2_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l2_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l2_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l4_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l4_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l4_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l4_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l9_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l9_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l9_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l9_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
