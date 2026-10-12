# Native Falcon grinding workload and dispatch equivalence

Read-only diagnosis of V5, 2026-10-06. This note does not establish a new performance gate result. The existing V5 Native128 batch-one qualification failed the total-prover gate; all four Native100 cells passed. Further timing belongs to the separate dispatch experiment.

## What changes between Native100 and Native128

Both profiles use the same arithmetic layout, the same sampled 126-bit prime family `[2^125, 2^126-1]`, and the split integer bridge with 8,192 rows. At batch one there are 16 arithmetic columns and source bit counts `[131072,1048576,524288]`. Their initial source commitment has the same geometry and rate. The archived seed42 cold runs have the same source root at both security targets.

The batch-one PCS ladder also has identical geometry:

| Level | Message columns | Lanes | Encoded rows | Rate | Queries at 100 | Queries at 128 |
| --- | ---: | ---: | ---: | --- | ---: | ---: |
| Initial | 1024 | 16 | 2048 | 1/2 | 245 | 313 |
| Middle | 128 | 8 | 512 | 1/4 | 152 | 194 |
| Final | 32 | 4 | 256 | 1/8 | 126 | 161 |

The compact final message is 128 GF elements (2,048 bytes) at both targets; its folded polynomial is 32 elements. Initial transport carries 13 occupied lanes at batch one. Query counts follow the code's finite-length UDR formula, not an asymptotic approximation.

At100, native prime and binary challenge grinding are absent. At128, batch-one difficulties are:

| Component | Bits |
| --- | ---: |
| Norm rounds | 14 |
| HashToPoint outer point | 12 |
| Product, compaction forest, and leaf sumcheck rounds | 16 |
| Forest line reductions | 14 |
| Fingerprint | 19 |
| Linear point | 13 |
| Arithmetic binding rounds | 14 |
| Native carry | 12 |
| Binary bridge / Keccak | 17 |
| Wiring / joint binary sumcheck / opening | 15 /14 /16 |

The existing seed42 candidate stage log records 394 `spartan:grinding` calls at 128: `{12:2,13:1,14:59,15:1,16:118,17:212,19:1}`. These are protocol-boundary counts, not executed hash counts. Its PCS `lig:grind_pow` calls are `{6:2,10:3,13:2,14:3,15:4}`; at 100, only three PCS query grinds at 6 bits remain. A stored nonce plus one describes the serial search prefix and excludes SIMD/parallel overscan.

Source locations: `hybrid.rs:116-162,336`, `layout.rs:112-125`, `piop.rs:49-50,74-110`, `native_ring.rs:150`, `hybrid_keccak.rs:199-225`, `ligerito_flock.rs:750-816`, and vendor `ligerito.rs:763-776`. Trace evidence: `performance/v5/stages/native-s{100,128}-b1-t16-seed42-candidate.stderr`.

## Which V5 changes can affect each timed stage

`PreparedFalconHybrid::commit` generates SHAKE witnesses, verification traces, arithmetic bits and packed sources, then calls `shared::commit_sources` for RS encoding and Merkle hashing. It does not call fixed-base exponentiation, the Keccak alpha transpose, or compact proof-row processing. Source commitment geometry/rate is identical between these two targets. No direct changed-code path explains a commitment-only regression.

The fixed-base public width optimization and cached generator table affect `falcon_hybrid:binary_bridge`; exponentiation is measured under `falcon_bridge:power_tables`. First-use cache preparation is immediately before that inner span, still inside the bridge. The same split widths and 8,192 exponent pairs are used at both targets.

The lane-contiguous Keccak transpose runs during both prover and verifier lincheck, twice per Falcon proof, within `falcon_hybrid:keccak_prefix`. It is not the Keccak witness generator. Compact initial rows and final message transport affect `falcon_hybrid:shared_opening`; the final 128-element clone is target-independent, while initial row count follows the query table above.

## Why changing the execution threshold preserves the proof

For a fixed public seed `s` and difficulty `b`, define

`n*(s,b) = min { n in [0,2^64-1] : leading_zero_bits(BLAKE3(s || LE64(n))) >= b }`.

The sequential SIMD routine scans ordered nonce batches and returns the first satisfying lane. The parallel routine assigns disjoint consecutive chunks, obtains each scanned chunk's smallest hit, and takes their minimum. Its shared monotone chunk counter guarantees all chunks below the returned hit were assigned; the pool barrier waits for them to finish. A stale relaxed load can cause extra work, but does not omit an earlier chunk. Therefore both return `n*`, provided their complete-domain endpoint behavior is the same.

Changing only the choice of these search implementations changes neither the seed, difficulty, minimum nonce, transcript absorption nor subsequent challenges. It leaves the soundness ledger and proof bytes unchanged. The threshold must depend only on public configuration, and an AVX512-specific policy must retain runtime feature detection and the prefix shape required by that kernel. A threshold is a performance policy; it does not reduce the required number of leading zero bits.

The existing Spartan sequential helper handles the final `u64::MAX` nonce explicitly after scanning the half-open range `0..u64::MAX`, matching the parallel helper. Zinc's existing direct sequential call omits that final fallback. A change to Zinc's threshold should preserve complete-domain behavior rather than claim unconditional equivalence while retaining this theoretical endpoint discrepancy. No such discrepancy was observed in proof runs, and it is not a soundness vulnerability.

Existing equality tests include `parallel_search_returns_the_serial_minimum` and `pool_search_preserves_minimum_across_threads_and_chunks` (1/2/4 workers, multiple prefix lengths and chunk sizes). The proposed isolated benchmark must compare returned nonces for every seed, and a production dispatch change needs boundary-difficulty coverage plus complete Falcon proof/transcript equivalence. Threshold measurements and integration checks are pending separately.

The subsequent isolated scanner experiment rejected raising the parallel threshold: serial search was slower for both seed widths throughout the measured range. See `README.md` in this directory for that experiment. No scanner policy change was made. The separately authorized compile-time schedule change is documented in `schedule-README.md`.
