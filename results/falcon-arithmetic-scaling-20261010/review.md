# Independent Rust review

No actionable correctness, aliasing, initialization, concurrency or protocol findings were found in the retained original cyclic candidate. Measured arithmetic and verification regressions remain; see [REPORT.md](REPORT.md). This source review does not establish strict nonregression.

The scope is the working change above `78c164f0fdb5939924561e2831241d7eb558c17b`: the new row dispatcher, forest integration and dense kernels. Exact restored source hashes are in [source-build.json](source-build.json). The Rust reviewer skill's protocol, concurrency, readability and performance guidance was applied. Locked Rayon-core 1.13.0 was inspected to verify current-pool `broadcast` behavior. No source edits or builds were performed by this reviewer.

## Ownership and initialization

`CyclicRows::new` accepts only power-of-two pools above 8 workers, a row count at least as large as and divisible by the worker count, and at least 4 MiB of touched matrix data. Each production caller constructs and consumes the dispatcher immediately in the same current pool. `broadcast` invokes its closure once per worker and waits for every invocation.

`map_indices` partitions indexes by residue modulo the worker count. `map_jobs` enumerates ordinary disjoint slice chunks on the caller and puts each job in exactly one worker bucket. The matching worker takes that bucket through its mutex, releasing the lock before the callback. Mutable jobs do not alias; no new unsafe reference construction or lock cycle is introduced.

Forest rebuilding preserves the original kernels and tails. The fused upper build retains the existing `RowsPtr` implementation: every base row is filled once, and descendant offsets preserve ownership because the gated row count is divisible by the worker count. Every broadcast completes before subsequent initialized reads or publication of the arena length. JIT jobs carry four disjoint output quarters. High-bit half and quarter offsets preserve worker ownership while the gate applies.

Dense dispatch requires multiple rows, so task width equals full row width and task index equals the absolute row index. Equality factors, endpoint modes and all left-weight modes are unchanged. Per-worker reduction reorders exact binary-field addition without changing proof messages, challenges, nonce choice, degree bounds or serialization.

## Allocation and feature gates

Large arenas, value tables and nibble rows remain caller-allocated. The dispatcher adds short-lived vectors of job references and per-worker results, without a private pool or persistent scratch cache. The whole forest is not entered through a new worker scope. The module and branches are gated on `parallel`; serial iterators remain unchanged without that feature. `falcon-hybrid` itself enables `parallel`.

All temporary inline attributes, coarse-write changes, whole-forest worker entry and the early shape-gate experiment were reverted. Every current candidate source hash matches the original cyclic freeze. The x86 backend remains unchanged from HEAD; no intrinsic arithmetic, SIMD safety contract or tail behavior changed. The packed streaming edit only enables an existing grouped-prefix test oracle without default features.

## Evidence and limitations

The retained source passed 76 parallel BitZ tests, 72 tests without default features, and one grouped-prefix oracle in each feature mode. Default development builds exclude the x86 SIMD module; these tests cover generic arithmetic and dispatch oracles. The original native holdout separately verified all 480 production trials across arithmetic/full Falcon and 1/4/8/16 workers. The unchanged-binary low-thread replication verified another 220 trials. Complete proof-debug digests match across configurations and worker counts for each variant and seed. Commands and log hashes are retained in [audit.json](audit.json).

The reviewer independently checked source and binary hashes, raw trial normalization, complete recorded argv, environment settings, observed affinities, actual protocol/proof identities and non-overlapping timed processes. The 4 MiB threshold is supported by measurements on this host and workload, not a universal claim. Compiler layout can affect fallback paths even where cyclic dispatch is disabled. The remaining measured regressions are recorded rather than inferred away.
