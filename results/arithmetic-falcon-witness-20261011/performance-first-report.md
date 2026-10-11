# Arithmetic Falcon performance-first qualification

**All 24 tested configurations pass both strict proving-time gates.** Proof-only medians improve by **0.21–4.07%**, and total proving medians by **0.18–3.05%**, against the original v5 baseline. Median proof payloads are **2.45–21.17% smaller**. The candidate uses compact witnesses above the minimum PCS allocation and retains the original signed15 representation at the floor, where compression would not save physical storage.

Verification remains a tradeoff: 18 of 24 verifier medians increase, and the full range is **1.33% faster to 2.43% slower**. All verification medians are recorded below. The proving gate covers both total proving and proof generation itself.

Both degrees, security 100/128, batches 1/32/1024, and one/eight threads were tested. The fixed small-case plan used seeds 42–73 (32 independent inputs per shape); large cases used seeds 42–49 (eight inputs). Each variant/input had one warmup and three measured proofs. All **3,456 measured proofs and 1,152 warmups verified**, and every baseline/candidate input digest matched. No seeds or shapes were selected after observing results.

All **768 Split proofs** shared with the preceding qualified run exactly preserve their source roots, full proof Debug digests, payload sizes, protocol IDs, inputs, trials, and physical source sizes. The **1,536 Single proofs** use the intended new floor layout; every input, verification result, protocol ID, trial, and physical source size was checked. Every reference case was required. See the [combined identity audit](performance-first-proof-identity.json) and its two detailed audit files.

Times are milliseconds. Positive Δ means slower; every proving delta below is negative. The primary gate uses the median of all measured trials in each shape: 96 samples per variant for small cases and 24 for large cases. A positive change in either metric fails the zero-tolerance observed gate.

| N | Security | Batch | Threads | Total before → after | Total Δ | Proof-only before → after | Proof Δ |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | 1 | 13.8214 → 13.6378 | -1.328% | 13.6563 → 13.4749 | -1.328% |
| 512 | 100 | 1 | 8 | 4.3839 → 4.3390 | -1.025% | 4.2257 → 4.1775 | -1.141% |
| 512 | 100 | 32 | 1 | 17.5739 → 17.5143 | -0.339% | 16.4076 → 16.3633 | -0.270% |
| 512 | 100 | 32 | 8 | 5.6235 → 5.5454 | -1.390% | 5.2736 → 5.2063 | -1.275% |
| 512 | 100 | 1024 | 1 | 270.3544 → 265.9789 | -1.618% | 235.0367 → 228.6793 | -2.705% |
| 512 | 100 | 1024 | 8 | 54.6264 → 53.1007 | -2.793% | 49.0450 → 47.4528 | -3.246% |
| 512 | 128 | 1 | 1 | 17.1840 → 17.0632 | -0.703% | 17.0198 → 16.9008 | -0.699% |
| 512 | 128 | 1 | 8 | 6.8348 → 6.7847 | -0.732% | 6.6702 → 6.6280 | -0.632% |
| 512 | 128 | 32 | 1 | 21.1010 → 21.0622 | -0.184% | 19.9512 → 19.9087 | -0.213% |
| 512 | 128 | 32 | 8 | 8.0957 → 8.0464 | -0.609% | 7.7156 → 7.6923 | -0.303% |
| 512 | 128 | 1024 | 1 | 281.8663 → 274.0331 | -2.779% | 246.6956 → 236.6455 | -4.074% |
| 512 | 128 | 1024 | 8 | 59.3637 → 58.1097 | -2.112% | 53.7907 → 52.4047 | -2.577% |
| 1024 | 100 | 1 | 1 | 13.9089 → 13.7729 | -0.978% | 13.6823 → 13.5471 | -0.988% |
| 1024 | 100 | 1 | 8 | 4.5402 → 4.4602 | -1.761% | 4.3176 → 4.2359 | -1.891% |
| 1024 | 100 | 32 | 1 | 26.9497 → 26.3961 | -2.054% | 23.6350 → 23.0034 | -2.672% |
| 1024 | 100 | 32 | 8 | 8.1031 → 7.8916 | -2.610% | 7.1942 → 7.0081 | -2.585% |
| 1024 | 100 | 1024 | 1 | 504.4318 → 495.2538 | -1.819% | 401.5912 → 389.9632 | -2.895% |
| 1024 | 100 | 1024 | 8 | 107.9522 → 104.8058 | -2.915% | 92.9863 → 89.8454 | -3.378% |
| 1024 | 128 | 1 | 1 | 17.3779 → 17.1756 | -1.164% | 17.1496 → 16.9520 | -1.152% |
| 1024 | 128 | 1 | 8 | 7.0554 → 7.0086 | -0.663% | 6.8341 → 6.7848 | -0.722% |
| 1024 | 128 | 32 | 1 | 31.0099 → 30.0638 | -3.051% | 27.6980 → 26.7126 | -3.558% |
| 1024 | 128 | 32 | 8 | 10.8269 → 10.5350 | -2.696% | 9.8449 → 9.5576 | -2.918% |
| 1024 | 128 | 1024 | 1 | 522.6186 → 507.8824 | -2.820% | 419.9347 → 403.3883 | -3.940% |
| 1024 | 128 | 1024 | 8 | 114.8771 → 111.8609 | -2.626% | 99.7793 → 96.7637 | -3.022% |

The minimum PCS source contains 524,288 bits (64 KiB). Small cases retain all signed15 coefficient bits and norm slack: 15,386 live bits per signature for N512 and 30,747 for N1024. Above the floor, compact maximum live counts are 12,338 and 24,651 bits (19.81% and 19.83% lower), and committed source bits decrease by 18.75%. These are committed witness counts, not total prover memory; the aligned arithmetic view remains. The arithmetic relation and integer norm semantics preserve every supplied coefficient exactly. SHAKE-256 and HashToPoint remain outside this arithmetic-only API.

| N | Batch | Source layout | Live bits/signature before → after | Physical source bytes before → after | Source reduction |
|---:|---:|---|---:|---:|---:|
| 512 | 1 | Single: signed15 floor | 15,386 → 15,386 | 65,536 → 65,536 | 0.00% |
| 512 | 32 | Single: signed15 floor | 15,386 → 15,386 | 65,536 → 65,536 | 0.00% |
| 512 | 1024 | Split: compact signed12 | 15,386 → 12,338 | 2,097,152 → 1,703,936 | 18.75% |
| 1024 | 1 | Single: signed15 floor | 30,747 → 30,747 | 65,536 → 65,536 | 0.00% |
| 1024 | 32 | Split: compact signed12 | 30,747 → 24,651 | 131,072 → 106,496 | 18.75% |
| 1024 | 1024 | Split: compact signed12 | 30,747 → 24,651 | 4,194,304 → 3,407,872 | 18.75% |

The final proof payload medians span **83,176–320,592 bytes**. Payload accounting includes stored proof values and PCS payload, excludes public inputs and transport framing, and does not specify a wire codec or Rust heap usage. Query/transcript differences can affect payload size across seeds, so the table reports medians. The removed batched ring certificate accounts for 11,242 bytes at N512 and 22,506 bytes at N1024 before other proof changes. Benchmark inputs used no compact coefficient exceptions; exception indices require `2 + 2*count` bytes per signature in Split mode, while Single has no exception metadata.

| N | Security | Batch | Threads | Verify before → after (ms) | Verify Δ | Payload before → after (bytes) | Payload reduction |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | 1 | 2.4988 → 2.5186 | +0.794% | 94,418 → 83,176 | 11.91% |
| 512 | 100 | 1 | 8 | 2.4913 → 2.5150 | +0.949% | 94,418 → 83,176 | 11.91% |
| 512 | 100 | 32 | 1 | 3.7078 → 3.7367 | +0.778% | 94,402 → 83,208 | 11.86% |
| 512 | 100 | 32 | 8 | 3.1985 → 3.1925 | -0.187% | 94,402 → 83,208 | 11.86% |
| 512 | 100 | 1024 | 1 | 44.0302 → 44.7377 | +1.607% | 225,402 → 217,520 | 3.50% |
| 512 | 100 | 1024 | 8 | 19.4841 → 19.5764 | +0.474% | 225,402 → 217,520 | 3.50% |
| 512 | 128 | 1 | 1 | 3.0617 → 3.0804 | +0.609% | 111,578 → 100,272 | 10.13% |
| 512 | 128 | 1 | 8 | 3.0612 → 3.0744 | +0.432% | 111,578 → 100,272 | 10.13% |
| 512 | 128 | 32 | 1 | 4.2690 → 4.3237 | +1.281% | 111,530 → 100,288 | 10.08% |
| 512 | 128 | 32 | 8 | 3.7560 → 3.7788 | +0.607% | 111,530 → 100,288 | 10.08% |
| 512 | 128 | 1024 | 1 | 44.4697 → 45.5523 | +2.434% | 263,858 → 257,400 | 2.45% |
| 512 | 128 | 1024 | 8 | 20.1602 → 20.1807 | +0.101% | 263,858 → 257,400 | 2.45% |
| 1024 | 100 | 1 | 1 | 2.6121 → 2.5775 | -1.327% | 105,714 → 83,336 | 21.17% |
| 1024 | 100 | 1 | 8 | 2.5911 → 2.5629 | -1.091% | 105,714 → 83,336 | 21.17% |
| 1024 | 100 | 32 | 1 | 5.1990 → 5.2318 | +0.630% | 118,026 → 104,968 | 11.06% |
| 1024 | 100 | 32 | 8 | 4.1836 → 4.1480 | -0.849% | 118,026 → 104,968 | 11.06% |
| 1024 | 100 | 1024 | 1 | 86.2002 → 88.1905 | +2.309% | 314,770 → 276,288 | 12.23% |
| 1024 | 100 | 1024 | 8 | 37.0804 → 36.8579 | -0.600% | 314,770 → 276,288 | 12.23% |
| 1024 | 128 | 1 | 1 | 3.1274 → 3.1380 | +0.341% | 122,858 → 100,432 | 18.25% |
| 1024 | 128 | 1 | 8 | 3.1012 → 3.1260 | +0.799% | 122,858 → 100,432 | 18.25% |
| 1024 | 128 | 32 | 1 | 5.7182 → 5.7913 | +1.278% | 137,730 → 127,656 | 7.31% |
| 1024 | 128 | 32 | 8 | 4.6824 → 4.6983 | +0.340% | 137,730 → 127,656 | 7.31% |
| 1024 | 128 | 1024 | 1 | 87.1358 → 88.6690 | +1.760% | 362,714 → 320,592 | 11.61% |
| 1024 | 128 | 1024 | 8 | 37.8770 → 37.7421 | -0.356% | 362,714 → 320,592 | 11.61% |

The supplemental analysis pairs each candidate/baseline input after taking the median of its three measurements. Bootstrap intervals resample whole seed pairs, with 10,000 replicates. Repetitions of one input repeat transcript-dependent prime-search and grinding work; they are not independent input samples. Positive deltas below mean slower. The paired ratios supplement the primary gate without replacing it. Intervals have no correction across shapes, and these measured improvements do not establish a universal speed guarantee.

| N | Security | Batch | Threads | Paired proof median Δ | Paired proof geometric mean Δ | Geometric mean 95% interval | Seeds faster |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | 1 | -0.737% | -1.275% | [-2.772%, -0.287%] | 26/32 |
| 512 | 100 | 1 | 8 | -0.985% | -0.804% | [-1.861%, +0.335%] | 21/32 |
| 512 | 100 | 32 | 1 | -0.287% | -0.292% | [-0.498%, -0.099%] | 22/32 |
| 512 | 100 | 32 | 8 | -0.665% | -1.054% | [-1.713%, -0.436%] | 20/32 |
| 512 | 100 | 1024 | 1 | -2.793% | -2.760% | [-3.076%, -2.440%] | 8/8 |
| 512 | 100 | 1024 | 8 | -3.358% | -3.151% | [-3.605%, -2.596%] | 8/8 |
| 512 | 128 | 1 | 1 | -0.032% | -0.186% | [-0.869%, +0.511%] | 17/32 |
| 512 | 128 | 1 | 8 | -0.707% | -0.681% | [-1.538%, +0.155%] | 20/32 |
| 512 | 128 | 32 | 1 | +0.259% | +0.190% | [-0.322%, +0.676%] | 13/32 |
| 512 | 128 | 32 | 8 | -0.752% | -0.286% | [-0.995%, +0.454%] | 20/32 |
| 512 | 128 | 1024 | 1 | -4.064% | -4.009% | [-4.352%, -3.638%] | 8/8 |
| 512 | 128 | 1024 | 8 | -2.442% | -2.478% | [-3.032%, -1.950%] | 8/8 |
| 1024 | 100 | 1 | 1 | -0.770% | -0.851% | [-1.183%, -0.523%] | 25/32 |
| 1024 | 100 | 1 | 8 | -2.726% | -2.763% | [-3.659%, -1.830%] | 27/32 |
| 1024 | 100 | 32 | 1 | -2.624% | -2.556% | [-2.799%, -2.286%] | 31/32 |
| 1024 | 100 | 32 | 8 | -2.576% | -2.695% | [-3.263%, -2.165%] | 31/32 |
| 1024 | 100 | 1024 | 1 | -2.839% | -2.811% | [-3.055%, -2.570%] | 8/8 |
| 1024 | 100 | 1024 | 8 | -3.277% | -3.124% | [-3.484%, -2.739%] | 8/8 |
| 1024 | 128 | 1 | 1 | -1.049% | -0.873% | [-1.402%, -0.344%] | 23/32 |
| 1024 | 128 | 1 | 8 | -1.224% | -0.781% | [-1.639%, +0.150%] | 22/32 |
| 1024 | 128 | 32 | 1 | -3.464% | -3.524% | [-4.005%, -3.042%] | 32/32 |
| 1024 | 128 | 32 | 8 | -3.265% | -3.062% | [-3.765%, -2.367%] | 30/32 |
| 1024 | 128 | 1024 | 1 | -3.764% | -4.113% | [-4.901%, -3.518%] | 8/8 |
| 1024 | 128 | 1024 | 8 | -3.250% | -3.362% | [-3.971%, -2.800%] | 8/8 |

Full paired ratios for both timing metrics, percentile dispersion, all per-seed values, and bootstrap intervals are in each run’s `paired-seed-analysis.json`. The untraced measurements do not separately attribute timing noise, prime sampling, or grinding. Small gains should be read at the precision and input coverage measured here.

The baseline is `/tmp/bitz-arithmetic-baseline-binary`; the frozen candidate is `/tmp/bitz-arithmetic-candidate-performance-first-binary` (SHA-256 `5208a4513f678c4bbd2ac0dafab83afda8971dd8fe9a4b634e371b9b80ea5273`). Both use locked/offline native release builds with `falcon-hybrid` and `RUSTFLAGS=-C target-cpu=native`. The machine is an AMD Ryzen 9 9950X3D. A single worker is pinned to CPU 6; eight workers use CPUs 6,7,4,5,2,3,0,1. Both Rayon pools use the requested count. Variants alternate order by seed, and processes run sequentially. No builds or tests ran during timing. Brief progress aggregation used CPU 15 outside benchmark workers; bootstrap and identity analysis ran after timing ended.

Total proving includes witness derivation, packing, commitment and proof generation. Input generation, decoding and external hashing are excluded. Diagnostic tracing is disabled. Binary hashes, CPU data, arguments, observed affinity, and all commands are retained in the run directories. Source provenance is in `baseline-source-sha256.json` and `candidate-performance-first-source-sha256.json`; the final build log is `performance-first-build.log`.

Artifacts: [small matrix](performance-first-small/), [large matrix](performance-first-large/), [combined summary](performance-first-summary.json), [combined proof audit](performance-first-proof-identity.json), [correctness validation](VALIDATION.md), and [implementation/witness summary](README.md). Each matrix retains `summary.json`, `seed-summary.json`, `samples.json`, `regression-gate.json`, `paired-seed-analysis.json`, `reference-proof-identity.json`, and every per-run command and log. Earlier failed candidates remain documented in [the pre-fallback report](v7-qualified-report.md) and [the original v6 report](v6-summary.md).
