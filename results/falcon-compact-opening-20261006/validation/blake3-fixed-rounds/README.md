# Fixed BLAKE3 round specialization

The isolated result supports retaining this focused change: 32-byte scanner latency and throughput improve about 10.3%; 16-byte scans improve about 8.5%. Full-prover impact remains subject to integrated qualification.

Production edit: `src/utils/blake3x4/avx512.rs`. The prepared 16-byte and 32-byte kernels replace the loop over the five public schedules with `round::<1>` through `round::<5>`. The helper retains the exact eight BLAKE3 G maps per round. Generic-prefix compression, nonce iteration, SIMD lane ordering/carry handling, difficulty checks, full-hash fallback, parallel dispatch, and security parameters are unchanged. No unchecked indexing or new memory operations were introduced.

## Correctness

- Both original and final production-copy scanner test suites ran in an isolated native build: 19 passed, 0 failed, 4 existing benchmarks ignored.
- Tests compare every SIMD first digest word against the independent `blake3` crate across all whole-word prefix lengths, multiple seeds, starts around 2^32 and near u64::MAX.
- Range tests cover empty/reversed ranges, unaligned starts, full and partial SIMD vectors, maximum end bounds, generic prefixes, and difficulties through 257.
- The new test forces every SIMD lane to be a potential candidate at difficulties 33, 64, 128, 256, 257, confirming the complete BLAKE3 digest predicate rejects false candidates. It also exercises final partial vectors and ranges ending at u64::MAX.
- Every one of 1,152 latency cases matches an independent ascending scalar BLAKE3 minimum-nonce oracle, including every one of 18,432 measured scans.

## Measurement

Main thread pinned to CPU 0, with readback; global 16-worker pool initialized and pinned to CPUs 0–15 consistently with prior diagnostics. Timed operations are sequential scanner calls on main CPU 0. 64 independent domain-separated prefixes per length, bits 12–20, four balanced ABBA/BAAB blocks per case, eight observations per method. Each length has an additional fixed-work pass over 2^18 nonces from an unaligned start, using impossible difficulty 257 to prevent early exit. No host settings were changed.

|Prefix bytes|Original full-range mean µs|Specialized mean µs|Ratio|Winning seeds|
|---:|---:|---:|---:|---:|
|16|1066.82|975.76|0.9146|64/64|
|32|1008.77|904.47|0.8966|64/64|

|Prefix bytes|Difficulty|Original latency mean µs|Specialized mean µs|Ratio|
|---:|---:|---:|---:|---:|
|16|12|16.14|14.77|0.9152|
|16|14|63.48|58.12|0.9156|
|16|16|218.89|200.18|0.9146|
|16|18|843.79|772.14|0.9151|
|16|20|3991.84|3649.45|0.9142|
|32|12|15.60|14.00|0.8974|
|32|14|74.67|67.00|0.8973|
|32|16|297.68|267.13|0.8974|
|32|18|1098.13|985.30|0.8973|
|32|20|4348.19|3896.89|0.8962|

## Code generation and provenance

The prepared-kernel round loops and their 16 dynamic index checks per round disappear; only the unchanged generic fallback retains its round loop. In this isolated build, the full dispatch function grows from 10,135 to 13,206 bytes (+3,071). Static stack references fall from 274 to 184; reserved stack size remains 0xb80 because the generic fallback is retained. Those figures are static code properties, not counts of executed memory operations. Raw disassembly and `codegen.json` are included.

The final helper uses the existing concise `word(i)` style. Rebuilding after that cleanup and final test-path adjustment produced a benchmark executable byte-identical to the measured executable, SHA256 `6ffa81ef3b65c5961d380d39b77db65869edd47cb2ca4f135e8a307860ff9781`. The final production source and isolated tested copy both have SHA256 `29db73960b0485315e041559951c4c117cf64cb6715f473db46d806f3931414a`.

Commands, toolchain and dependency/source hashes: `method.json`. Raw timing rows: `raw.csv` and `throughput.csv`. Prefixes and oracle minima: `cases.csv`. Final correctness output: `tests-final.log`. Affinity and completion evidence: `run.log`.
