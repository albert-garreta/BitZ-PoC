Implemented prepared Montgomery coordinate collapse, four interleaved AVX-512 power/adjoint pairs, and an N-point negacyclic NTT for algebraic witness preparation. Small batches retain individual recurrences when grouping would leave workers without a task. Canonical coordinates, integer carries, transcript order, proof contents, and full Falcon's ordinary-product API are preserved.

Validation used the frozen post-Albert baseline `b30f5e445`. The five-file change is recorded in `final.patch`, with source and binary hashes in `provenance.json`. Existing main-thread edits were preserved. A separate integration run included the concurrent 16-worker binding change and passed all 121 algebraic tests; the timing tables below isolate the ring changes against the frozen baseline.

At Falcon-1024, batch 1024, security 128, eight workers, diagnostic ring time changed from **12.564 ms to 11.205 ms** (about 11% faster). Coordinate generation changed from 6.675 to 6.263 ms and collapse from 4.888 to 3.836 ms. These are medians across three seeds, each with one warmup and three traced proofs. Tracing changes absolute latency; use the untraced measurements below for whole-proof performance.

Total prover time includes witness preparation and commitment. The table shows medians of per-seed process medians for batch 1024; percentage changes use the geometric mean of paired seed ratios.

| Degree | Security | Workers | Baseline total ms | Candidate total ms | Paired change |
|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | 153.947 | 145.732 | -5.30% |
| 512 | 100 | 8 | 33.493 | 32.375 | -3.67% |
| 512 | 100 | 16 | 46.784 | 46.452 | -0.84% |
| 512 | 128 | 1 | 161.308 | 153.551 | -5.23% |
| 512 | 128 | 8 | 37.258 | 36.089 | -2.81% |
| 512 | 128 | 16 | 50.518 | 50.036 | -0.69% |
| 1024 | 100 | 1 | 298.370 | 281.805 | -5.51% |
| 1024 | 100 | 8 | 65.992 | 63.650 | -3.53% |
| 1024 | 100 | 16 | 91.992 | 91.018 | -1.03% |
| 1024 | 128 | 1 | 309.394 | 293.438 | -5.16% |
| 1024 | 128 | 8 | 68.765 | 67.057 | -2.35% |
| 1024 | 128 | 16 | 96.268 | 95.468 | -0.89% |

The final algebraic matrix covers degrees 512/1024, batches 1/3/4/8/32/513/1024, security 100/128, and one/eight/sixteen workers: 84 configurations, three seeds, two warmups and five measured proofs per variant. Variant order alternates and workers are pinned. No builds or correctness tests ran during timing. Input digests, source roots, complete-proof debug digests, payload sizes, and payload breakdowns match in every pair. Debug digests compare exact builds; they are not a stable wire encoding.

Four flagged configurations were repeated using eight fresh seeds, three warmups and nine measured proofs. None retained a slowdown above the 1% investigation threshold. One tiny case initially showed proof-only +0.39%; another eight fresh seeds with five warmups and fifteen measurements yielded +0.04%, with an interval spanning zero. This is finite-sample evidence, not a claim that all possible workloads have zero regression. Full-Falcon controls also matched proof identities, with no total/proof slowdown above 1%; apparent control speedups are not attributed to the algebraic ring algorithms.

**606 test executions passed.** Across initial, final, confirmation and full-control campaigns, all **6,824 proof executions** verified and all **568 pairs** matched identities. See `validation.json` and the logs for exact suites. The no-parallel library test build has a pre-existing unguarded Rayon reference in `src/sumcheck/inner/packed/streaming.rs:3000`; the no-parallel library check, portable field tests and generic-CPU one-worker algebraic suite pass.

The recurrence tests compare native and portable paths with independent extension arithmetic, maximum and random coefficients, empty/short/long lengths, malformed dimensions, and guarded output buffers. Collapse tests compare individual field operations at boundary coordinates and signed carry limits. NTT tests compare both an independent schoolbook product and the original ordinary NTT, including full i16/u16 ranges, negative wraparound, and reused storage. Existing proof and transcript tampering tests remain enabled.

Kernel benchmarks include output stores and compare variants in alternating order. Prepared collapse uses a runtime 126-bit probable prime and 2,097,152 eleven-term dot products; recurrence and NTT comparisons process 1,024 signatures. They show roughly 29% lower collapse cost, 52–64% lower recurrence cost with four streams, and 42% lower NTT cost. These gains apply to the kernels, not the entire ring stage. Further multi-limb SIMD collapse and wider redundant residues remain conditional experiments; neither is enabled by this change.

Run the standalone kernel harness with `RUSTFLAGS="-C target-cpu=native" cargo run --release --manifest-path results/falcon-ring-optimizations-20261012/kernel-bench/Cargo.toml -- adjoint` (or `ntt`, or no argument for collapse). Run it without concurrent builds or tests. `qualify.py` contains the paired proof runner; campaign metadata records binaries, CPU placement and sample counts. `raw-evidence.tar.gz` preserves raw commands, proof outputs, diagnostic traces, assembly and build logs, relative to the evidence directory.
