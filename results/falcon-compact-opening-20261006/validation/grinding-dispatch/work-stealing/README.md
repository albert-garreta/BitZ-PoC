# Work-stealing dispatch prototype

Recommendation: keep the existing broadcast dispatcher. The simple parallel-iterator prototype is slower across every tested difficulty; no production change is justified.

The isolated prototype replaces only `rayon::broadcast` with `(0..rayon::current_num_threads()).into_par_iter().for_each`. It retains the identical atomics, worker loop, chunk-size rule, SIMD scanner, complete difficulty, and final `u64::MAX` check. Both implementations finish all assigned work before returning the minimum.

The same 64 independent 32-byte prefixes from the prior experiment were tested at bits 12–20. Each case used four balanced ABBA/BAAB blocks, for eight observations per method. The independent ascending scalar BLAKE3 oracle matched all 576 cases and all 9,216 timed calls. Main thread was pinned to CPU 0; 16 Rayon workers were pinned to CPUs 0–15 with readback.

|Bits|Broadcast mean µs|Iterator mean µs|Iterator/broadcast|Broadcast p95 µs|Iterator p95 µs|
|---:|---:|---:|---:|---:|---:|
|12|6.29|7.88|1.254|7.53|9.96|
|13|7.46|9.18|1.230|11.51|13.72|
|14|9.77|11.81|1.210|20.62|22.85|
|15|16.41|18.86|1.149|43.38|46.14|
|16|25.13|27.34|1.088|57.39|58.90|
|17|41.79|43.93|1.051|100.59|103.26|
|18|78.68|81.14|1.031|204.19|207.38|
|19|155.21|158.06|1.018|423.05|427.38|
|20|295.39|298.75|1.011|1066.88|1071.70|

Means and 95th percentiles above use per-seed averages of eight observations. The iterator adds roughly 1.6–3.4µs per call and loses on at least 62 of 64 seeds at every difficulty. Its per-call 99th percentiles are also higher in every cell; those values and maxima are in `summary.json`. No 26 ms scheduling outlier appeared during this bounded run, so it cannot establish whether either dispatcher is more resilient to rare host interruptions. The observed routine cost favors the existing dispatcher.

Do not compare absolute microseconds against the earlier sequential/broadcast experiment as a before/after change: this run alternates two parallel methods, while the earlier run interleaves a long serial scan. The conclusions use paired comparisons within each experiment.

Exact build/run command and hashes are in `method.json`; source is the sibling `bench-work-stealing.rs`, raw timings are `raw.csv`, oracle minima and prefixes are `cases.csv`, and affinity/completion evidence is `run.log`. Production sources remain unchanged.
