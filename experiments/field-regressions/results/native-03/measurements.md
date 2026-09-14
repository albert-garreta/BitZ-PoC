# Field regression measurements

Relative time is candidate / baseline; lower is faster. CIs are pointwise 95% hierarchical paired bootstrap intervals.
The 5% operational margin is configurable, not a proof of zero slowdown. Inconclusive results block promotion.
No production implementation was replaced. Latencies are averages per measured batch, not individual-call tail latency.

| Workload | Size | Variant | Median (µs) | P10–P90 (µs) | Relative time [95% CI] | Gate |
|---|---|---|---:|---:|---:|---|
| chain | 1024 | flock | 8.126 | 8.050–8.353 | 1.000 [1.000, 1.000] | baseline |
| chain | 1024 | karatsuba | 8.920 | 8.807–9.176 | 1.094 [1.091, 1.098] | regression |
| chain | 1024 | karatsuba_barrett | 9.963 | 9.866–10.194 | 1.227 [1.220, 1.230] | regression |
| chain | 1024 | schoolbook | 10.295 | 10.134–10.522 | 1.262 [1.256, 1.266] | regression |
| chain | 1024 | shared | 7.878 | 7.806–8.118 | 0.970 [0.966, 0.973] | within margin |
| dot | 1024 | flock | 0.860 | 0.849–0.875 | 1.000 [1.000, 1.000] | baseline |
| dot | 1024 | shared | 0.861 | 0.850–0.879 | 1.003 [0.994, 1.009] | within margin |
| dot | 1024 | vec2 | 1.716 | 1.692–1.734 | 1.994 [1.981, 1.999] | regression |
| dot | 1024 | wide1 | 0.698 | 0.687–0.721 | 0.813 [0.810, 0.817] | within margin |
| dot | 1024 | wide16 | 1.102 | 1.089–1.122 | 1.283 [1.277, 1.288] | regression |
| dot | 1024 | wide2 | 0.698 | 0.689–0.718 | 0.812 [0.809, 0.819] | within margin |
| dot | 1024 | wide4 | 0.698 | 0.689–0.720 | 0.813 [0.809, 0.817] | within margin |
| dot | 1024 | wide8 | 0.739 | 0.729–0.756 | 0.860 [0.857, 0.867] | within margin |
| dot | 1048576 | flock | 891.068 | 884.292–911.776 | 1.000 [1.000, 1.000] | baseline |
| dot | 1048576 | shared | 890.599 | 884.016–920.583 | 1.001 [0.998, 1.007] | within margin |
| dot | 1048576 | vec2 | 1764.234 | 1758.708–1783.239 | 1.984 [1.979, 1.987] | regression |
| dot | 1048576 | wide1 | 723.568 | 717.698–751.776 | 0.813 [0.807, 0.817] | within margin |
| dot | 1048576 | wide16 | 1112.661 | 1104.693–1153.125 | 1.252 [1.247, 1.255] | regression |
| dot | 1048576 | wide2 | 723.797 | 717.062–762.651 | 0.813 [0.810, 0.821] | within margin |
| dot | 1048576 | wide4 | 724.302 | 717.849–765.542 | 0.814 [0.811, 0.819] | within margin |
| dot | 1048576 | wide8 | 768.760 | 758.854–797.943 | 0.861 [0.858, 0.865] | within margin |
| dot | 16 | flock | 0.015 | 0.015–0.016 | 1.000 [1.000, 1.000] | baseline |
| dot | 16 | shared | 0.015 | 0.015–0.016 | 1.002 [0.993, 1.008] | within margin |
| dot | 16 | vec2 | 0.029 | 0.028–0.030 | 1.868 [1.861, 1.885] | regression |
| dot | 16 | wide1 | 0.013 | 0.013–0.014 | 0.875 [0.863, 0.882] | within margin |
| dot | 16 | wide16 | 0.032 | 0.031–0.033 | 2.070 [2.062, 2.088] | regression |
| dot | 16 | wide2 | 0.014 | 0.014–0.015 | 0.905 [0.892, 0.914] | within margin |
| dot | 16 | wide4 | 0.014 | 0.014–0.015 | 0.915 [0.908, 0.923] | within margin |
| dot | 16 | wide8 | 0.015 | 0.015–0.016 | 1.007 [1.000, 1.015] | within margin |
| dot | 65536 | flock | 55.104 | 54.436–56.321 | 1.000 [1.000, 1.000] | baseline |
| dot | 65536 | shared | 55.076 | 54.460–56.368 | 1.000 [0.994, 1.007] | within margin |
| dot | 65536 | vec2 | 109.940 | 108.637–111.649 | 1.992 [1.984, 2.005] | regression |
| dot | 65536 | wide1 | 44.502 | 43.993–45.478 | 0.806 [0.803, 0.812] | within margin |
| dot | 65536 | wide16 | 69.702 | 68.846–70.615 | 1.263 [1.255, 1.271] | regression |
| dot | 65536 | wide2 | 44.356 | 44.019–45.332 | 0.808 [0.799, 0.811] | within margin |
| dot | 65536 | wide4 | 44.451 | 44.033–45.856 | 0.809 [0.806, 0.814] | within margin |
| dot | 65536 | wide8 | 47.154 | 46.691–48.402 | 0.855 [0.850, 0.861] | within margin |
| ntt | log12_lanes8 | flock | 410.839 | 401.888–428.453 | 1.000 [1.000, 1.000] | baseline |
| ntt | log12_lanes8 | prepared_twiddle | 385.396 | 377.896–398.578 | 0.938 [0.928, 0.947] | within margin |
| ntt | log12_lanes8 | reset_only | 8.758 | 8.424–9.943 | 0.021 [0.021, 0.022] | overhead diagnostic |
| ntt | log12_lanes8 | shared_generic | 385.346 | 376.318–400.453 | 0.938 [0.931, 0.952] | within margin |
| ntt | log12_lanes8 | shared_twiddle | 397.706 | 389.779–413.430 | 0.967 [0.955, 0.983] | within margin |
| ntt | log15_lanes32 | flock | 11186.875 | 11019.791–11375.521 | 1.000 [1.000, 1.000] | baseline |
| ntt | log15_lanes32 | prepared_twiddle | 12591.896 | 12288.229–15167.167 | 1.123 [1.103, 1.266] | regression |
| ntt | log15_lanes32 | reset_only | 372.896 | 356.188–439.229 | 0.033 [0.033, 0.035] | overhead diagnostic |
| ntt | log15_lanes32 | shared_generic | 12667.834 | 12404.792–15309.063 | 1.129 [1.113, 1.230] | regression |
| ntt | log15_lanes32 | shared_twiddle | 13034.875 | 12632.000–14708.562 | 1.156 [1.142, 1.245] | regression |
| ntt | log8_lanes1 | flock | 2.579 | 2.514–2.687 | 1.000 [1.000, 1.000] | baseline |
| ntt | log8_lanes1 | prepared_twiddle | 2.376 | 2.312–2.445 | 0.916 [0.909, 0.925] | within margin |
| ntt | log8_lanes1 | reset_only | 0.054 | 0.053–0.118 | 0.021 [0.021, 0.022] | overhead diagnostic |
| ntt | log8_lanes1 | shared_generic | 2.309 | 2.244–2.398 | 0.897 [0.879, 0.907] | within margin |
| ntt | log8_lanes1 | shared_twiddle | 2.437 | 2.366–2.534 | 0.942 [0.929, 0.955] | within margin |
| ntt | log8_lanes32 | flock | 46.591 | 45.799–47.558 | 1.000 [1.000, 1.000] | baseline |
| ntt | log8_lanes32 | prepared_twiddle | 40.001 | 39.061–41.079 | 0.859 [0.850, 0.863] | within margin |
| ntt | log8_lanes32 | reset_only | 2.133 | 2.000–2.296 | 0.046 [0.045, 0.047] | overhead diagnostic |
| ntt | log8_lanes32 | shared_generic | 44.158 | 43.399–45.495 | 0.953 [0.941, 0.957] | within margin |
| ntt | log8_lanes32 | shared_twiddle | 40.100 | 39.150–41.207 | 0.861 [0.852, 0.865] | within margin |
| products | 1024 | flock | 0.869 | 0.855–0.889 | 1.000 [1.000, 1.000] | baseline |
| products | 1024 | karatsuba | 3.083 | 3.036–3.128 | 3.554 [3.532, 3.571] | regression |
| products | 1024 | karatsuba_barrett | 1.616 | 1.598–1.646 | 1.865 [1.855, 1.871] | regression |
| products | 1024 | schoolbook | 2.633 | 2.604–2.687 | 3.042 [3.023, 3.052] | regression |
| products | 1024 | shared | 0.923 | 0.910–0.941 | 1.067 [1.059, 1.070] | regression |
| products | 1024 | shared_unroll2 | 0.923 | 0.908–0.945 | 1.064 [1.058, 1.072] | regression |
| products | 1024 | shared_unroll4 | 0.923 | 0.907–0.948 | 1.064 [1.054, 1.069] | regression |
| products | 1024 | shared_unroll8 | 0.923 | 0.907–0.943 | 1.062 [1.057, 1.069] | regression |
| products | 1048576 | flock | 901.490 | 892.812–937.958 | 1.000 [1.000, 1.000] | baseline |
| products | 1048576 | karatsuba | 3163.276 | 3154.917–3238.302 | 3.519 [3.502, 3.534] | regression |
| products | 1048576 | karatsuba_barrett | 1706.312 | 1699.130–1757.630 | 1.896 [1.883, 1.902] | regression |
| products | 1048576 | schoolbook | 2703.646 | 2696.380–2781.635 | 3.008 [2.991, 3.018] | regression |
| products | 1048576 | shared | 956.542 | 948.146–1014.839 | 1.063 [1.058, 1.068] | regression |
| products | 1048576 | shared_unroll2 | 954.240 | 946.906–1008.797 | 1.059 [1.055, 1.066] | regression |
| products | 1048576 | shared_unroll4 | 957.005 | 946.630–996.838 | 1.062 [1.055, 1.067] | regression |
| products | 1048576 | shared_unroll8 | 956.224 | 948.042–1026.578 | 1.063 [1.060, 1.068] | regression |
| products | 16 | flock | 0.016 | 0.016–0.017 | 1.000 [1.000, 1.000] | baseline |
| products | 16 | karatsuba | 0.051 | 0.050–0.052 | 3.143 [3.123, 3.148] | regression |
| products | 16 | karatsuba_barrett | 0.028 | 0.028–0.029 | 1.749 [1.741, 1.756] | regression |
| products | 16 | schoolbook | 0.043 | 0.043–0.044 | 2.672 [2.663, 2.687] | regression |
| products | 16 | shared | 0.017 | 0.017–0.017 | 1.032 [1.027, 1.036] | within margin |
| products | 16 | shared_unroll2 | 0.017 | 0.017–0.017 | 1.055 [1.050, 1.062] | regression |
| products | 16 | shared_unroll4 | 0.017 | 0.017–0.018 | 1.070 [1.066, 1.076] | regression |
| products | 16 | shared_unroll8 | 0.017 | 0.017–0.018 | 1.080 [1.074, 1.082] | regression |
| products | 65536 | flock | 55.248 | 54.639–56.327 | 1.000 [1.000, 1.000] | baseline |
| products | 65536 | karatsuba | 197.975 | 196.193–201.864 | 3.588 [3.570, 3.610] | regression |
| products | 65536 | karatsuba_barrett | 105.177 | 104.259–107.022 | 1.902 [1.898, 1.913] | regression |
| products | 65536 | schoolbook | 168.779 | 167.393–171.801 | 3.049 [3.040, 3.068] | regression |
| products | 65536 | shared | 58.765 | 58.248–60.503 | 1.067 [1.060, 1.071] | regression |
| products | 65536 | shared_unroll2 | 58.887 | 58.272–60.067 | 1.061 [1.056, 1.070] | regression |
| products | 65536 | shared_unroll4 | 58.721 | 58.236–60.259 | 1.067 [1.054, 1.071] | regression |
| products | 65536 | shared_unroll8 | 58.976 | 58.356–60.475 | 1.068 [1.062, 1.074] | regression |
