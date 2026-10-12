# Grinding dispatch microbenchmark

Recommendation: retain the existing parallel-search threshold of 12 bits. Do not move short AVX-512 Spartan searches to sequential dispatch based on this workload. No production scanner code was changed.

This is an isolated scanner measurement, not a full-prover performance qualification. Both methods invoke the current production kernels, with byte-identical source snapshots in this directory. The parallel helper keeps its existing chunk size (256 nonces for AVX-512 through 16 bits, 1024 thereafter).

## Method

- Native `rustc` release-style build, exact command/toolchain/dependency hashes in `method.json` and `dependencies.json`.
- Global Rayon pool: 16 workers, one each on CPUs 0–15; main thread on CPU 0. Each thread’s affinity was read back. No host settings changed.
- 64 independent deterministic prefixes per prefix length, lengths 32 (Spartan) and 16 (Ligerito control), difficulties 12–20. Prefixes are domain-separated BLAKE3 hashes of length and seed index.
- For each case, two balanced ABBA/BAAB blocks, yielding four timings of each method. Seed order is permuted deterministically between blocks. Allocation, logging, and nonce assertions are outside the timed calls.
- The independent oracle hashes every prefix||nonce_le in ascending order until the first 20-bit hit, collecting first hits at every difficulty 12–20. Both methods match that oracle for all 1,152 cases and all 9,216 measured calls. Sequential helper includes the final u64::MAX fallback used by Spartan.
- Summary means and 95th percentiles are calculated across 64 per-seed means (four observations each).

## 32-byte Spartan results

|Difficulty bits|Sequential mean µs|Parallel mean µs|Parallel/serial|Sequential p95 µs|Parallel p95 µs|
|---:|---:|---:|---:|---:|---:|
|12|15.53|9.83|0.633|47.52|14.61|
|13|35.10|23.91|0.681|100.28|18.55|
|14|74.38|18.82|0.253|235.41|35.37|
|15|174.38|24.17|0.139|575.72|51.34|
|16|298.55|33.88|0.113|771.38|65.89|
|17|538.50|56.34|0.105|1455.03|125.43|
|18|1100.26|116.14|0.106|3031.24|315.52|
|19|2257.90|232.41|0.103|6337.13|608.74|
|20|4357.61|393.93|0.090|16014.15|1289.05|

## 16-byte control results

|Difficulty bits|Sequential mean µs|Parallel mean µs|Parallel/serial|Sequential p95 µs|Parallel p95 µs|
|---:|---:|---:|---:|---:|---:|
|12|15.96|9.74|0.611|45.11|14.60|
|13|30.85|11.79|0.382|97.02|17.85|
|14|63.35|15.62|0.247|191.87|23.97|
|15|113.49|19.47|0.172|334.98|34.91|
|16|219.83|28.20|0.128|647.91|56.98|
|17|505.42|56.77|0.112|1605.27|193.49|
|18|847.39|131.06|0.155|2353.03|532.50|
|19|1716.30|193.54|0.113|5232.71|563.27|
|20|4007.68|380.50|0.095|10908.52|880.75|

Parallel dispatch has a fixed scheduling cost, so individual seeds with very small minimum nonces can favor sequential scanning. The aggregate result is decisive in the important 14–17-bit region: parallel means are 74–90% lower for 32-byte prefixes. A threshold increase would substantially increase expected proving work there. A few scheduling outliers affect small-cell means (especially 32-byte/13-bit); even including them, parallel is faster on average.

This benchmark preserves the complete difficulty and minimum-nonce policy. It does not justify weakening grinding, changing security budgets, selecting a threshold from secret data, or changing other platforms. It also does not measure possible new hybrid scanners; no such implementation is proposed here.

Raw per-call observations are in `raw.csv`; prefixes and independently verified minima are in `cases.csv`; affinity/completion evidence is in `run.log`.
