# Field regression measurements

Relative time is candidate / baseline; lower is faster. CIs are pointwise 95% hierarchical paired bootstrap intervals.
The 5% operational margin is configurable, not a proof of zero slowdown. Inconclusive results block promotion.
No production implementation was replaced. Latencies are averages per measured batch, not individual-call tail latency.

| Workload | Size | Variant | Median (µs) | P10–P90 (µs) | Relative time [95% CI] | Gate |
|---|---|---|---:|---:|---:|---|
| chain | 1024 | flock | 8.131 | 8.053–8.322 | 1.000 [1.000, 1.000] | baseline |
| chain | 1024 | karatsuba | 8.886 | 8.814–9.082 | 1.093 [1.089, 1.099] | regression |
| chain | 1024 | karatsuba_barrett | 9.958 | 9.867–10.122 | 1.226 [1.216, 1.229] | regression |
| chain | 1024 | scalar_lanes | 8.039 | 7.962–8.135 | 0.988 [0.984, 0.989] | within margin |
| chain | 1024 | schoolbook | 10.249 | 10.122–10.389 | 1.257 [1.253, 1.260] | regression |
| chain | 1024 | shared | 8.045 | 7.951–8.197 | 0.987 [0.983, 0.992] | within margin |
| dot | 1024 | flock | 0.867 | 0.858–0.885 | 1.000 [1.000, 1.000] | baseline |
| dot | 1024 | scalar_lanes | 0.867 | 0.855–0.884 | 1.000 [0.996, 1.007] | within margin |
| dot | 1024 | shared | 0.867 | 0.858–0.890 | 1.002 [0.995, 1.009] | within margin |
| dot | 1024 | vec2 | 1.720 | 1.704–1.754 | 1.983 [1.973, 2.001] | regression |
| dot | 1024 | wide1 | 0.701 | 0.695–0.719 | 0.809 [0.805, 0.814] | within margin |
| dot | 1024 | wide16 | 1.106 | 1.094–1.132 | 1.281 [1.271, 1.286] | regression |
| dot | 1024 | wide2 | 0.701 | 0.695–0.714 | 0.810 [0.806, 0.812] | within margin |
| dot | 1024 | wide4 | 0.703 | 0.696–0.717 | 0.810 [0.807, 0.816] | within margin |
| dot | 1024 | wide8 | 0.744 | 0.736–0.758 | 0.859 [0.853, 0.864] | within margin |
| dot | 1048576 | flock | 892.292 | 885.578–929.401 | 1.000 [1.000, 1.000] | baseline |
| dot | 1048576 | scalar_lanes | 890.630 | 885.713–925.135 | 0.997 [0.994, 1.001] | within margin |
| dot | 1048576 | shared | 890.203 | 885.411–918.448 | 0.997 [0.993, 1.001] | within margin |
| dot | 1048576 | vec2 | 1766.510 | 1760.140–1805.630 | 1.981 [1.975, 1.986] | regression |
| dot | 1048576 | wide1 | 723.562 | 718.167–760.646 | 0.811 [0.808, 0.815] | within margin |
| dot | 1048576 | wide16 | 1110.682 | 1105.188–1135.151 | 1.245 [1.242, 1.249] | regression |
| dot | 1048576 | wide2 | 723.380 | 718.870–767.667 | 0.811 [0.810, 0.815] | within margin |
| dot | 1048576 | wide4 | 723.177 | 718.391–756.427 | 0.811 [0.810, 0.814] | within margin |
| dot | 1048576 | wide8 | 764.729 | 760.359–801.281 | 0.858 [0.856, 0.862] | within margin |
| dot | 16 | flock | 0.015 | 0.015–0.016 | 1.000 [1.000, 1.000] | baseline |
| dot | 16 | scalar_lanes | 0.015 | 0.015–0.016 | 1.001 [0.990, 1.005] | within margin |
| dot | 16 | shared | 0.015 | 0.015–0.016 | 1.001 [0.995, 1.008] | within margin |
| dot | 16 | vec2 | 0.029 | 0.028–0.030 | 1.876 [1.857, 1.883] | regression |
| dot | 16 | wide1 | 0.013 | 0.013–0.014 | 0.874 [0.869, 0.881] | within margin |
| dot | 16 | wide16 | 0.032 | 0.031–0.032 | 2.073 [2.045, 2.085] | regression |
| dot | 16 | wide2 | 0.014 | 0.014–0.014 | 0.902 [0.893, 0.906] | within margin |
| dot | 16 | wide4 | 0.014 | 0.014–0.014 | 0.912 [0.907, 0.919] | within margin |
| dot | 16 | wide8 | 0.015 | 0.015–0.016 | 1.006 [0.996, 1.012] | within margin |
| dot | 65536 | flock | 55.423 | 54.544–56.630 | 1.000 [1.000, 1.000] | baseline |
| dot | 65536 | scalar_lanes | 55.285 | 54.595–57.418 | 1.002 [0.993, 1.012] | within margin |
| dot | 65536 | shared | 55.432 | 54.628–57.107 | 0.999 [0.992, 1.010] | within margin |
| dot | 65536 | vec2 | 110.114 | 109.306–113.479 | 1.991 [1.981, 2.000] | regression |
| dot | 65536 | wide1 | 44.582 | 44.060–45.909 | 0.803 [0.800, 0.814] | within margin |
| dot | 65536 | wide16 | 69.845 | 68.931–72.233 | 1.260 [1.252, 1.271] | regression |
| dot | 65536 | wide2 | 44.721 | 44.075–46.321 | 0.806 [0.801, 0.811] | within margin |
| dot | 65536 | wide4 | 44.830 | 44.096–46.299 | 0.808 [0.805, 0.815] | within margin |
| dot | 65536 | wide8 | 47.419 | 46.762–49.270 | 0.856 [0.854, 0.862] | within margin |
| ntt | log12_lanes8 | flock | 409.258 | 400.201–420.568 | 1.000 [1.000, 1.000] | baseline |
| ntt | log12_lanes8 | prepared_twiddle | 382.925 | 375.503–392.289 | 0.938 [0.930, 0.946] | within margin |
| ntt | log12_lanes8 | reset_only | 8.615 | 8.435–10.349 | 0.021 [0.021, 0.022] | overhead diagnostic |
| ntt | log12_lanes8 | shared_generic | 382.289 | 375.375–393.575 | 0.937 [0.929, 0.945] | within margin |
| ntt | log12_lanes8 | shared_twiddle | 395.792 | 386.706–405.706 | 0.968 [0.960, 0.977] | within margin |
| ntt | log15_lanes32 | flock | 11150.709 | 11010.021–11288.688 | 1.000 [1.000, 1.000] | baseline |
| ntt | log15_lanes32 | prepared_twiddle | 12454.750 | 12303.479–15443.166 | 1.112 [1.110, 1.249] | regression |
| ntt | log15_lanes32 | reset_only | 370.521 | 352.041–414.375 | 0.033 [0.033, 0.034] | overhead diagnostic |
| ntt | log15_lanes32 | shared_generic | 12584.250 | 12436.792–15359.979 | 1.122 [1.120, 1.252] | regression |
| ntt | log15_lanes32 | shared_twiddle | 12805.667 | 12650.312–14709.188 | 1.145 [1.142, 1.299] | regression |
| ntt | log8_lanes1 | flock | 2.580 | 2.524–2.665 | 1.000 [1.000, 1.000] | baseline |
| ntt | log8_lanes1 | prepared_twiddle | 2.366 | 2.310–2.425 | 0.920 [0.905, 0.922] | within margin |
| ntt | log8_lanes1 | reset_only | 0.055 | 0.053–0.134 | 0.021 [0.021, 0.022] | overhead diagnostic |
| ntt | log8_lanes1 | shared_generic | 2.295 | 2.245–2.412 | 0.893 [0.884, 0.901] | within margin |
| ntt | log8_lanes1 | shared_twiddle | 2.431 | 2.365–2.497 | 0.944 [0.933, 0.949] | within margin |
| ntt | log8_lanes32 | flock | 46.174 | 45.350–46.751 | 1.000 [1.000, 1.000] | baseline |
| ntt | log8_lanes32 | prepared_twiddle | 39.375 | 38.737–40.191 | 0.856 [0.850, 0.861] | within margin |
| ntt | log8_lanes32 | reset_only | 2.051 | 1.943–2.172 | 0.045 [0.043, 0.046] | overhead diagnostic |
| ntt | log8_lanes32 | shared_generic | 43.747 | 42.881–44.538 | 0.949 [0.945, 0.954] | within margin |
| ntt | log8_lanes32 | shared_twiddle | 39.685 | 38.834–40.371 | 0.861 [0.856, 0.867] | within margin |
| products | 1024 | flock | 0.866 | 0.853–0.887 | 1.000 [1.000, 1.000] | baseline |
| products | 1024 | karatsuba | 3.076 | 3.039–3.142 | 3.553 [3.522, 3.585] | regression |
| products | 1024 | karatsuba_barrett | 1.619 | 1.596–1.642 | 1.872 [1.855, 1.880] | regression |
| products | 1024 | scalar_lanes | 0.867 | 0.854–0.888 | 1.000 [0.992, 1.008] | within margin |
| products | 1024 | schoolbook | 2.632 | 2.597–2.659 | 3.031 [3.015, 3.065] | regression |
| products | 1024 | shared | 0.918 | 0.909–0.938 | 1.062 [1.056, 1.075] | regression |
| products | 1024 | shared_unroll2 | 0.918 | 0.906–0.941 | 1.059 [1.052, 1.069] | regression |
| products | 1024 | shared_unroll4 | 0.922 | 0.910–0.937 | 1.062 [1.055, 1.075] | regression |
| products | 1024 | shared_unroll8 | 0.921 | 0.908–0.941 | 1.062 [1.055, 1.076] | regression |
| products | 1048576 | flock | 902.385 | 895.213–916.208 | 1.000 [1.000, 1.000] | baseline |
| products | 1048576 | karatsuba | 3172.026 | 3158.943–3188.838 | 3.509 [3.503, 3.531] | regression |
| products | 1048576 | karatsuba_barrett | 1710.214 | 1702.266–1725.875 | 1.896 [1.887, 1.900] | regression |
| products | 1048576 | scalar_lanes | 902.453 | 894.099–912.448 | 1.000 [0.996, 1.002] | within margin |
| products | 1048576 | schoolbook | 2711.312 | 2698.995–2733.292 | 3.008 [2.994, 3.015] | regression |
| products | 1048576 | shared | 956.062 | 949.713–975.890 | 1.062 [1.055, 1.065] | regression |
| products | 1048576 | shared_unroll2 | 956.880 | 949.453–978.912 | 1.061 [1.055, 1.066] | regression |
| products | 1048576 | shared_unroll4 | 954.807 | 949.167–978.094 | 1.062 [1.055, 1.064] | regression |
| products | 1048576 | shared_unroll8 | 958.479 | 948.463–986.932 | 1.061 [1.057, 1.065] | regression |
| products | 16 | flock | 0.016 | 0.016–0.017 | 1.000 [1.000, 1.000] | baseline |
| products | 16 | karatsuba | 0.051 | 0.050–0.052 | 3.127 [3.091, 3.149] | regression |
| products | 16 | karatsuba_barrett | 0.029 | 0.028–0.029 | 1.764 [1.754, 1.776] | regression |
| products | 16 | scalar_lanes | 0.016 | 0.016–0.017 | 1.000 [0.995, 1.003] | within margin |
| products | 16 | schoolbook | 0.043 | 0.043–0.044 | 2.665 [2.646, 2.686] | regression |
| products | 16 | shared | 0.017 | 0.016–0.017 | 1.022 [1.014, 1.028] | within margin |
| products | 16 | shared_unroll2 | 0.017 | 0.017–0.017 | 1.054 [1.047, 1.060] | inconclusive |
| products | 16 | shared_unroll4 | 0.017 | 0.017–0.018 | 1.065 [1.060, 1.072] | regression |
| products | 16 | shared_unroll8 | 0.018 | 0.017–0.018 | 1.075 [1.067, 1.087] | regression |
| products | 65536 | flock | 55.845 | 54.828–57.063 | 1.000 [1.000, 1.000] | baseline |
| products | 65536 | karatsuba | 199.227 | 197.737–202.915 | 3.574 [3.556, 3.596] | regression |
| products | 65536 | karatsuba_barrett | 105.816 | 104.551–107.836 | 1.897 [1.879, 1.913] | regression |
| products | 65536 | scalar_lanes | 55.743 | 54.762–57.011 | 0.998 [0.989, 1.006] | within margin |
| products | 65536 | schoolbook | 169.931 | 168.467–172.894 | 3.045 [3.021, 3.070] | regression |
| products | 65536 | shared | 59.273 | 58.262–60.503 | 1.061 [1.053, 1.069] | regression |
| products | 65536 | shared_unroll2 | 59.222 | 58.349–60.909 | 1.066 [1.052, 1.073] | regression |
| products | 65536 | shared_unroll4 | 59.281 | 58.330–60.626 | 1.064 [1.055, 1.069] | regression |
| products | 65536 | shared_unroll8 | 59.173 | 58.411–60.765 | 1.059 [1.051, 1.069] | regression |
