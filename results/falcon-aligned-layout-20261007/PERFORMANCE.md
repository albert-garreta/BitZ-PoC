# Aligned Falcon layout performance

The algebraic prover improves substantially. Full Falcon does **not** show a reliable binding speedup: single-thread binding is slightly slower and 16-thread binding is slightly faster. Every measured full-prover total is slower than its baseline. The largest regression is dominated by directly measured PCS grinding after the protocol changes its transcript challenges.

The main comparison contains 480 verified proofs: two provers, batches 32 and 1,024, targets 100 and 128, and 1, 2, 4, 8, 16 workers, with one warm-up and five measured trials per baseline/candidate pair. Runs were sequential and used matching original Falcon-1024 inputs, seed 42, native release builds, and pinned physical cores on `will`. The separate tracing run contains 96 verified proofs and is not mixed into throughput results. Its coverage is 128-bit security, batches 32 and 1,024, and 1 and 16 workers.

## Total proving and binding

Across the main comparison, algebraic total proving time falls by **22.2–58.3% at batch 32** and **37.8–64.9% at batch 1,024**. For 1,024 signatures at 128 bits:

| Workers | Algebraic total before / after (ms) | Full total before / after (ms) |
|---:|---:|---:|
| 1 | 2,411.596 / 877.199 | 6,431.045 / 7,418.437 |
| 2 | 1,323.523 / 532.836 | 3,412.955 / 3,907.441 |
| 4 | 788.428 / 362.449 | 1,931.145 / 2,171.420 |
| 8 | 516.734 / 274.633 | 1,193.688 / 1,316.708 |
| 16 | 403.234 / 250.761 | 889.796 / 956.755 |

The traced binding medians are:

| Prover | Batch | Workers | Before (ms) | After (ms) |
|---|---:|---:|---:|---:|
| Algebraic | 32 | 1 | 51.100130 | 8.290141 |
| Algebraic | 32 | 16 | 5.790230 | 2.210140 |
| Algebraic | 1,024 | 1 | 1,620.000250 | 248.000170 |
| Algebraic | 1,024 | 16 | 159.000150 | 47.700160 |
| Full | 32 | 1 | 52.336211 | 53.380437 |
| Full | 32 | 16 | 9.973161 | 9.601047 |
| Full | 1,024 | 1 | 1,571.310773 | 1,593.121534 |
| Full | 1,024 | 16 | 131.022151 | 129.265357 |

These are the medians recorded in `binding-diagnostics/summary.json`. Algebraic durations come from rounded human-readable tracing output, so their displayed decimal digits do not imply that measurement precision. Compare before/after **within each prover**: `falcon_algebraic:binding` and `falcon_arithmetic:binding_inner` have different boundaries. Tracing also adds overhead relative to the main runs.

## Why full Falcon is slower in this run

Full total proving increases **0.7–7.0% at batch 32**. At batch 1,024 it increases **0.2–1.4% at 100 bits** and **7.5–15.4% at 128 bits**. The latter change is much larger than the observed full binding change.

For batch 1,024 at 128 bits, exact-name stage events give the following medians. The PCS grinding row sums `lig:grind_pow` spans within each proof's shared opening before taking the median; it is a child of the shared-opening row and must not be added to that row.

| Traced stage | 1 worker before / after (ms) | 16 workers before / after (ms) |
|---|---:|---:|
| Arithmetic prefix | 2,486.710 / 2,510.849 | 273.635 / 271.555 |
| Binary bridge | 647.155 / 683.515 | 170.296 / 173.577 |
| Keccak prefix | 358.001 / 357.238 | 98.181 / 98.547 |
| Link coefficient construction | 2.408 / 3.099 | 2.539 / 3.042 |
| Joint sumcheck | 294.462 / 303.684 | 48.062 / 49.107 |
| Shared opening | 1,961.155 / 2,860.400 | 159.575 / 220.733 |
| PCS proof-of-work grinding inside opening | 1,274.774 / 2,172.096 | 85.842 / 146.402 |
| Complete traced proof generation | 5,787.598 / 6,756.517 | 790.561 / 853.087 |

The extra **897.322 ms** of PCS grinding at one worker accounts for most of the **968.919 ms** traced proof-generation increase; at 16 workers the corresponding increases are **60.560 ms** and **62.527 ms**. This locates the dominant regression in measured grinding time, rather than inferring it solely from nonce values. Main untraced total deltas also include preparation and were measured in separate runs.

The fixed-input PCS-fold nonce-prefix sum changes from **344,959,272 to 589,195,169**. This is the sum of stored `nonce + 1` values, **not an executed-hash count**; it excludes parallel/SIMD overscan. Reusing the same input seed does not hold grinding work constant when layout versions and transcript challenges change. These measurements therefore do not establish an average 15% full-prover regression across independent inputs.

Full Falcon also now proves that internal, trailing, and inactive arithmetic padding is zero. This adds work to link construction and the joint sumcheck and increases the link grinding budget. Their measured changes above include existing work and changed challenges too; they are not isolated measurements of padding enforcement. No general full-prover speedup should be claimed from this experiment.

## Proof payload and memory

| Prover | Batch | Target | Payload before / after (bytes) |
|---|---:|---:|---:|
| Algebraic | 32 | 100 | 134,586 / 119,082 |
| Algebraic | 32 | 128 | 156,410 / 138,202 |
| Algebraic | 1,024 | 100 | 418,970 / 316,114 |
| Algebraic | 1,024 | 128 | 472,890 / 364,290 |
| Full | 32 | 100 | 260,246 / 259,734 |
| Full | 32 | 128 | 322,258 / 319,602 |
| Full | 1,024 | 100 | 652,622 / 655,566 |
| Full | 1,024 | 128 | 814,554 / 813,690 |

Payloads are independent of worker count in these runs. Algebraic payload includes its 32-byte commitment root; full payload excludes the separately supplied root. Both exclude public inputs and outer transport framing. Full payload changes occur in Merkle authentication components as transcript-selected query overlaps change; they do not demonstrate a structural proof-size reduction.

Algebraic packed source storage halves: **256 to 128 KiB** at batch 32, and **8 to 4 MiB** at batch 1,024. For the 1,024-signature, 128-bit, one-worker run, process peak RSS falls from **383.10 to 291.67 MiB**. Full packed source storage is unchanged at **5.5 MiB** and **176 MiB** respectively, including the existing Keccak sources. Full peak RSS remains approximately 3.2 GiB at batch 1,024. RSS includes input setup, all process trials, and persistent caches; it is not isolated witness memory.

Evidence: `summary.json`, raw before/after JSONL files, `binding-diagnostics/summary.json`, and the full diagnostic `.stderr` stage logs in the same result directory. No additional seed run is required to report these fixed-input observations; an average performance claim across inputs would require separate sampling of grinding workloads.
