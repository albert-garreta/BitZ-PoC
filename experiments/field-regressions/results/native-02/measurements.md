# Field regression measurements

Relative time is candidate / baseline; lower is faster. CIs are pointwise 95% hierarchical paired bootstrap intervals.
The 5% operational margin is configurable, not a proof of zero slowdown. Inconclusive results block promotion.
No production implementation was replaced. Latencies are averages per measured batch, not individual-call tail latency.

| Workload | Size | Variant | Median (µs) | P10–P90 (µs) | Relative time [95% CI] | Gate |
|---|---|---|---:|---:|---:|---|
| chain | 1024 | flock | 8.225 | 8.061–8.530 | 1.000 [1.000, 1.000] | baseline |
| chain | 1024 | karatsuba | 9.012 | 8.862–9.307 | 1.095 [1.090, 1.104] | regression |
| chain | 1024 | karatsuba_barrett | 10.116 | 9.944–10.520 | 1.230 [1.224, 1.236] | regression |
| chain | 1024 | schoolbook | 10.398 | 10.211–10.785 | 1.259 [1.251, 1.264] | regression |
| chain | 1024 | shared | 7.987 | 7.824–8.346 | 0.971 [0.966, 0.979] | within margin |
| dot | 1024 | flock | 0.867 | 0.859–0.896 | 1.000 [1.000, 1.000] | baseline |
| dot | 1024 | shared | 0.870 | 0.858–0.886 | 0.999 [0.996, 1.005] | within margin |
| dot | 1024 | vec2 | 1.722 | 1.708–1.754 | 1.991 [1.972, 1.997] | regression |
| dot | 1024 | wide1 | 0.701 | 0.694–0.731 | 0.807 [0.801, 0.815] | within margin |
| dot | 1024 | wide16 | 1.108 | 1.098–1.133 | 1.276 [1.268, 1.284] | regression |
| dot | 1024 | wide2 | 0.703 | 0.695–0.725 | 0.809 [0.803, 0.818] | within margin |
| dot | 1024 | wide4 | 0.699 | 0.695–0.720 | 0.808 [0.803, 0.812] | within margin |
| dot | 1024 | wide8 | 0.745 | 0.736–0.771 | 0.858 [0.853, 0.866] | within margin |
| dot | 1048576 | flock | 889.016 | 882.459–930.403 | 1.000 [1.000, 1.000] | baseline |
| dot | 1048576 | shared | 889.953 | 881.825–925.227 | 1.001 [0.990, 1.005] | within margin |
| dot | 1048576 | vec2 | 1773.031 | 1759.052–1818.583 | 1.985 [1.950, 1.995] | regression |
| dot | 1048576 | wide1 | 724.776 | 717.672–766.555 | 0.812 [0.805, 0.818] | within margin |
| dot | 1048576 | wide16 | 1110.240 | 1104.824–1162.537 | 1.252 [1.237, 1.255] | regression |
| dot | 1048576 | wide2 | 723.505 | 717.465–765.876 | 0.813 [0.810, 0.817] | within margin |
| dot | 1048576 | wide4 | 724.802 | 718.220–767.509 | 0.814 [0.803, 0.819] | within margin |
| dot | 1048576 | wide8 | 765.151 | 759.858–808.822 | 0.862 [0.846, 0.867] | within margin |
| dot | 16 | flock | 0.015 | 0.015–0.016 | 1.000 [1.000, 1.000] | baseline |
| dot | 16 | shared | 0.015 | 0.015–0.016 | 0.998 [0.992, 1.009] | within margin |
| dot | 16 | vec2 | 0.029 | 0.028–0.030 | 1.879 [1.859, 1.899] | regression |
| dot | 16 | wide1 | 0.013 | 0.013–0.014 | 0.873 [0.869, 0.877] | within margin |
| dot | 16 | wide16 | 0.032 | 0.032–0.032 | 2.091 [2.066, 2.096] | regression |
| dot | 16 | wide2 | 0.014 | 0.014–0.014 | 0.901 [0.896, 0.904] | within margin |
| dot | 16 | wide4 | 0.014 | 0.014–0.015 | 0.917 [0.909, 0.919] | within margin |
| dot | 16 | wide8 | 0.015 | 0.015–0.016 | 1.005 [0.998, 1.013] | within margin |
| dot | 65536 | flock | 54.994 | 54.536–56.307 | 1.000 [1.000, 1.000] | baseline |
| dot | 65536 | shared | 54.829 | 54.510–56.085 | 0.996 [0.989, 1.004] | within margin |
| dot | 65536 | vec2 | 109.684 | 109.051–111.081 | 1.994 [1.982, 2.004] | regression |
| dot | 65536 | wide1 | 44.601 | 44.065–45.800 | 0.809 [0.802, 0.817] | within margin |
| dot | 65536 | wide16 | 69.422 | 68.804–71.011 | 1.261 [1.252, 1.268] | regression |
| dot | 65536 | wide2 | 44.321 | 44.036–44.984 | 0.807 [0.800, 0.809] | within margin |
| dot | 65536 | wide4 | 44.379 | 44.042–45.651 | 0.807 [0.803, 0.814] | within margin |
| dot | 65536 | wide8 | 47.082 | 46.666–47.847 | 0.856 [0.848, 0.865] | within margin |
| matrix | rows21183_entries4241586 | indexed_u16 | 21455.729 | 21065.324–21811.574 | 1.024 [1.019, 1.033] | within margin |
| matrix | rows21183_entries4241586 | indexed_u32 | 20956.563 | 20501.946–21332.471 | 1.000 [1.000, 1.000] | baseline |
| matrix | rows21183_entries4241586 | inline_aos | 18802.688 | 18481.183–19046.588 | 0.897 [0.892, 0.902] | within margin |
| matrix | rows21183_entries4241586 | inline_soa | 18890.459 | 18665.621–19515.983 | 0.905 [0.900, 0.909] | within margin |
| matrix | rows256_entries20986 | indexed_u16 | 105.183 | 103.529–107.284 | 1.046 [1.034, 1.050] | within margin |
| matrix | rows256_entries20986 | indexed_u32 | 100.630 | 98.547–102.983 | 1.000 [1.000, 1.000] | baseline |
| matrix | rows256_entries20986 | inline_aos | 90.430 | 88.951–92.318 | 0.903 [0.898, 0.907] | within margin |
| matrix | rows256_entries20986 | inline_soa | 91.307 | 89.674–92.839 | 0.909 [0.901, 0.912] | within margin |
| matrix | rows8_entries512 | indexed_u16 | 2.572 | 2.525–2.610 | 1.047 [1.045, 1.049] | within margin |
| matrix | rows8_entries512 | indexed_u32 | 2.456 | 2.408–2.486 | 1.000 [1.000, 1.000] | baseline |
| matrix | rows8_entries512 | inline_aos | 2.175 | 2.130–2.208 | 0.882 [0.881, 0.887] | within margin |
| matrix | rows8_entries512 | inline_soa | 2.202 | 2.165–2.235 | 0.898 [0.895, 0.900] | within margin |
| ntt | log12_lanes8 | flock | 412.112 | 405.309–422.628 | 1.000 [1.000, 1.000] | baseline |
| ntt | log12_lanes8 | prepared_twiddle | 383.659 | 377.770–402.933 | 0.928 [0.906, 0.966] | within margin |
| ntt | log12_lanes8 | reset_only | 8.461 | 8.276–9.273 | 0.021 [0.020, 0.021] | overhead diagnostic |
| ntt | log12_lanes8 | shared_generic | 385.656 | 380.310–399.942 | 0.936 [0.921, 0.949] | within margin |
| ntt | log12_lanes8 | shared_twiddle | 400.430 | 388.663–410.964 | 0.967 [0.957, 0.981] | within margin |
| ntt | log15_lanes32 | flock | 11119.188 | 10945.571–11409.934 | 1.000 [1.000, 1.000] | baseline |
| ntt | log15_lanes32 | prepared_twiddle | 12355.875 | 12265.567–12727.172 | 1.121 [1.110, 1.131] | regression |
| ntt | log15_lanes32 | reset_only | 373.166 | 359.716–424.867 | 0.034 [0.033, 0.034] | overhead diagnostic |
| ntt | log15_lanes32 | shared_generic | 12560.688 | 12390.912–12947.491 | 1.135 [1.125, 1.141] | regression |
| ntt | log15_lanes32 | shared_twiddle | 12804.334 | 12655.758–13127.075 | 1.153 [1.145, 1.162] | regression |
| ntt | log8_lanes1 | flock | 2.586 | 2.529–2.852 | 1.000 [1.000, 1.000] | baseline |
| ntt | log8_lanes1 | prepared_twiddle | 2.383 | 2.337–2.506 | 0.919 [0.910, 0.929] | within margin |
| ntt | log8_lanes1 | reset_only | 0.055 | 0.053–0.118 | 0.021 [0.021, 0.021] | overhead diagnostic |
| ntt | log8_lanes1 | shared_generic | 2.319 | 2.266–2.512 | 0.895 [0.889, 0.903] | within margin |
| ntt | log8_lanes1 | shared_twiddle | 2.461 | 2.417–2.608 | 0.951 [0.934, 0.961] | within margin |
| ntt | log8_lanes32 | flock | 46.264 | 45.831–47.125 | 1.000 [1.000, 1.000] | baseline |
| ntt | log8_lanes32 | prepared_twiddle | 39.521 | 39.000–40.261 | 0.855 [0.852, 0.858] | within margin |
| ntt | log8_lanes32 | reset_only | 1.986 | 1.935–2.185 | 0.043 [0.042, 0.045] | overhead diagnostic |
| ntt | log8_lanes32 | shared_generic | 43.669 | 43.265–44.507 | 0.948 [0.942, 0.951] | within margin |
| ntt | log8_lanes32 | shared_twiddle | 39.501 | 39.164–40.150 | 0.854 [0.850, 0.858] | within margin |
| products | 1024 | flock | 0.869 | 0.858–0.889 | 1.000 [1.000, 1.000] | baseline |
| products | 1024 | karatsuba | 3.081 | 3.055–3.124 | 3.555 [3.534, 3.570] | regression |
| products | 1024 | karatsuba_barrett | 1.616 | 1.603–1.640 | 1.866 [1.850, 1.885] | regression |
| products | 1024 | schoolbook | 2.639 | 2.616–2.667 | 3.043 [3.016, 3.054] | regression |
| products | 1024 | shared | 0.920 | 0.912–0.945 | 1.060 [1.049, 1.072] | inconclusive |
| products | 1048576 | flock | 897.823 | 891.471–946.220 | 1.000 [1.000, 1.000] | baseline |
| products | 1048576 | karatsuba | 3161.844 | 3155.920–3344.143 | 3.533 [3.518, 3.539] | regression |
| products | 1048576 | karatsuba_barrett | 1705.880 | 1699.345–1825.310 | 1.905 [1.894, 1.910] | regression |
| products | 1048576 | schoolbook | 2702.797 | 2694.849–2829.831 | 3.018 [3.009, 3.023] | regression |
| products | 1048576 | shared | 953.807 | 948.193–1058.660 | 1.066 [1.058, 1.070] | regression |
| products | 16 | flock | 0.017 | 0.016–0.017 | 1.000 [1.000, 1.000] | baseline |
| products | 16 | karatsuba | 0.052 | 0.051–0.056 | 3.157 [3.127, 3.172] | regression |
| products | 16 | karatsuba_barrett | 0.028 | 0.028–0.031 | 1.735 [1.723, 1.747] | regression |
| products | 16 | schoolbook | 0.044 | 0.043–0.047 | 2.677 [2.663, 2.699] | regression |
| products | 16 | shared | 0.017 | 0.017–0.018 | 1.036 [1.021, 1.042] | within margin |
| products | 65536 | flock | 55.317 | 54.678–56.868 | 1.000 [1.000, 1.000] | baseline |
| products | 65536 | karatsuba | 198.151 | 197.162–201.771 | 3.590 [3.551, 3.608] | regression |
| products | 65536 | karatsuba_barrett | 105.341 | 104.399–107.745 | 1.909 [1.884, 1.920] | regression |
| products | 65536 | schoolbook | 169.029 | 168.214–171.388 | 3.051 [3.030, 3.082] | regression |
| products | 65536 | shared | 59.161 | 58.214–60.724 | 1.061 [1.056, 1.077] | regression |
| public_coefficient_reduction | complete | per_entry | 1157195.375 | 1144618.562–1190259.875 | 1113.127 [1100.282, 1124.754] | regression |
| public_coefficient_reduction | complete | pool_once | 1046.792 | 1018.021–1091.374 | 1.000 [1.000, 1.000] | baseline |
