# SHA-256 / P-256 comparison using shared Spartan kernels

All **64 configurations** completed and all **256 proofs**, including warmups,
were encoded, decoded and verified. Every fixture matched across methods and
chunkings. The sweep covers 2^i SHA-256 compressions for every i from 4 through
11, with one P-256 ECDSA verification per proof, at 1 and 32 threads.

The Spartan backend is commit [`3b941304`](https://github.com/wu-s-john/Spartan2/commit/3b94130453838550a1086ee75c5d280f5c25df3d).
Its non-ZK adapter reuses the existing NeutronNova folding arithmetic and
batched outer/inner sumcheck kernels. The non-ZK transcript, native verifier,
SHA/ECDSA circuit layout and direct Hyrax opening still differ from the stock
Vega ZK path. These are measurements of this integration, not a reproduction
of published Vega timings. The [previous bf99f4f8 sweep](sha256-ecdsa-i4-i11-results.md)
and [initial results](sha256-ecdsa-comparison-results.md) describe the historical
custom implementation before this refactor.

Each configuration uses one warmup and three measured repetitions with
`seed=0`. Each metric is a median of the three measured samples; memory is the
whole-worker high-water mark. The two main tables use **F2Z Split / Spartan**
in every cell, with Spartan `r=0,c=i` (one compression per SHA instance).
All times are **ms**, proof sizes **KiB**, and peak resident memory **MiB**.

## 32 threads

| Compressions | Commit | Witness | PIOP | IOP/PCS | Verify | E2E | Proof KiB | Peak MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 | 0.75 / 19.45 | 4.29 / 10.10 | 748.71 / 39.24 | 28.31 / 0.32 | 736.65 / 17.11 | 1,518.89 / 87.09 | 144.93 / 97.88 | 1,228 / 464 |
| 32 | 0.75 / 23.03 | 4.33 / 12.31 | 745.92 / 55.78 | 31.02 / 0.29 | 736.15 / 18.48 | 1,520.61 / 108.45 | 146.53 / 122.51 | 1,233 / 544 |
| 64 | 0.74 / 25.57 | 4.69 / 17.58 | 749.24 / 85.17 | 30.82 / 0.28 | 736.27 / 20.42 | 1,522.65 / 149.77 | 152.26 / 171.63 | 1,227 / 747 |
| 128 | 0.75 / 36.82 | 5.37 / 25.49 | 745.96 / 154.09 | 36.14 / 0.30 | 728.74 / 22.53 | 1,516.62 / 240.63 | 162.45 / 269.76 | 1,225 / 1,036 |
| 256 | 0.90 / 58.10 | 6.04 / 43.98 | 751.74 / 296.69 | 44.70 / 0.30 | 728.47 / 25.83 | 1,531.06 / 424.85 | 176.37 / 465.88 | 1,249 / 1,778 |
| 512 | 0.91 / 102.66 | 8.58 / 80.22 | 766.21 / 528.37 | 59.73 / 0.29 | 735.69 / 33.20 | 1,570.85 / 746.62 | 221.29 / 858.01 | 1,286 / 3,238 |
| 1,024 | 1.05 / 190.90 | 12.62 / 153.79 | 773.29 / 901.74 | 78.95 / 0.30 | 715.27 / 47.28 | 1,580.38 / 1,293.71 | 219.77 / 1,642.13 | 1,351 / 6,177 |
| 2,048 | 1.73 / 366.58 | 20.23 / 297.63 | 827.09 / 1,908.50 | 120.53 / 0.29 | 729.25 / 75.13 | 1,697.62 / 2,648.16 | 278.49 / 3,210.26 | 1,564 / 12,080 |

## 1 thread

| Compressions | Commit | Witness | PIOP | IOP/PCS | Verify | E2E | Proof KiB | Peak MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 | 1.14 / 155.28 | 14.97 / 50.43 | 835.39 / 91.11 | 75.67 / 0.48 | 751.89 / 26.23 | 1,680.20 / 322.97 | 144.93 / 97.88 | 1,222 / 343 |
| 32 | 0.79 / 204.97 | 17.35 / 94.27 | 839.99 / 153.64 | 82.16 / 0.49 | 745.98 / 29.43 | 1,680.44 / 483.06 | 146.53 / 122.51 | 1,223 / 436 |
| 64 | 1.12 / 301.89 | 22.31 / 193.16 | 865.24 / 272.01 | 90.69 / 0.49 | 744.92 / 34.30 | 1,724.37 / 801.24 | 152.26 / 171.63 | 1,220 / 619 |
| 128 | 1.10 / 497.99 | 32.38 / 368.45 | 889.63 / 499.84 | 114.99 / 0.50 | 730.14 / 43.47 | 1,769.24 / 1,409.06 | 162.45 / 269.76 | 1,223 / 988 |
| 256 | 2.23 / 891.27 | 54.70 / 721.11 | 992.74 / 932.08 | 162.61 / 0.48 | 733.82 / 59.13 | 1,946.63 / 2,606.19 | 176.37 / 465.88 | 1,246 / 1,724 |
| 512 | 2.82 / 1,677.12 | 96.42 / 1,411.13 | 1,201.01 / 1,788.85 | 255.88 / 0.65 | 765.57 / 87.39 | 2,324.77 / 4,966.99 | 221.29 / 858.01 | 1,280 / 3,198 |
| 1,024 | 3.13 / 3,243.89 | 180.88 / 2,848.54 | 1,582.38 / 3,393.88 | 444.17 / 0.48 | 769.75 / 140.58 | 2,980.70 / 9,632.80 | 219.77 / 1,642.13 | 1,324 / 6,148 |
| 2,048 | 3.28 / 6,386.15 | 347.43 / 5,619.51 | 2,383.80 / 6,713.65 | 838.47 / 0.48 | 829.30 / 236.75 | 4,400.92 / 18,984.37 | 278.49 / 3,210.26 | 1,505 / 12,046 |

## Change from the earlier Spartan adapter

For 2,048 compressions with `r=0,c=11`, commitment-plus-proving time (ms):

| Threads | Earlier bf99f4f8 | Shared kernels |
| ---: | ---: | ---: |
| 1 | 16,977.19 | 13,100.30 |
| 32 | 1,646.57 | 2,275.40 |

The change improves the single-thread result but slows the 32-thread result.
At 32 threads, folding decreases from 939.69
to 618.42 ms, while matrix/preparation increases
from 257.71 to 1,216.30 ms.
The new matrix/preparation phase includes fresh signed small-value cache
construction. That preparation is currently the largest measured protocol
phase at this configuration. These observations concern this adapter and
its fresh-witness accounting, not the published Vega benchmark.

## Timing and proof boundaries

- **Witness:** fresh witness generation, including chain states and signature hints.
- **Commit:** the F2Z source commitment, or all Spartan SHA/core commitments.
- **PIOP:** `protocol_ms - opening_ms`, computed separately for each sample.
  This includes preparation, projection/matrix work, folding, sumchecks and
  transcript overhead outside the opening stage.
- **IOP/PCS:** `opening_ms`; the complete F2Z virtual opening path or Spartan's
  combined witness/blind construction and direct Hyrax opening. F2Z's opening
  input preparation remains in PIOP. This is a timing category, not an extra
  oracle protocol in Spartan.
- **Verify:** complete application verification, including chain links,
  padding and public input binding.
- **E2E:** `median(witness_to_proof_ms + verify_ms)`, with the addition performed
  on each measured run. This includes witness generation, commitment, PIOP,
  opening and verification. It excludes setup, serialization/deserialization,
  fixture generation and signing. It is not the sum of separately computed
  column medians.
- **Proof:** all encoded proof material, including commitments and auxiliary
  public inputs carried by Spartan's proof. The expected 129-byte statement
  `(i, Qx, Qy, r_sig, s_sig)` is additional.
- **Peak:** GNU time's maximum resident memory for the whole worker, including
  setup, warmup, all measured repetitions and verification. It is not a
  separately sampled per-proof peak; the same value is repeated for that
  worker in the sample CSV. The main tables round it to the nearest MiB.

For each sample, `commit + PIOP + IOP/PCS = prove`. Setup and codec times are
recorded separately in the CSV. Every message has `64 * (2^i - 1)` bytes; its
`2^i` compressions include one final padding compression. The message and
digest are witnesses, with one signature verified against the final digest.

## Additional configurations

The capped Spartan policy uses `r=min(4,i-3), c=i-r`: eight instances through
128 compressions, then 16 compressions per instance. F2Z uses only total
`i=r+c`; it is measured once per mode and thread count. These two Spartan
policies do not constitute an exhaustive search over chunkings.

The following prover totals are medians of `prove_ms`: commitment + PIOP +
IOP/PCS, **excluding witness generation and verification**. All values are ms.

| Threads | Compressions | F2Z Split | F2Z AllRows | Spartan B=1 | Spartan capped B≤16 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 32 | 16 | 777.77 | 728.50 | 59.53 | 81.36 |
| 32 | 32 | 777.16 | 721.64 | 79.09 | 165.46 |
| 32 | 64 | 780.80 | 744.04 | 111.78 | 318.69 |
| 32 | 128 | 782.37 | 786.14 | 192.25 | 642.45 |
| 32 | 256 | 797.35 | 776.89 | 355.25 | 823.66 |
| 32 | 512 | 826.88 | 840.49 | 634.06 | 1,157.16 |
| 32 | 1,024 | 854.28 | 852.55 | 1,092.95 | 1,822.47 |
| 32 | 2,048 | 949.17 | 1,052.45 | 2,275.40 | 3,073.84 |
| 1 | 16 | 912.68 | 849.44 | 246.76 | 273.18 |
| 1 | 32 | 922.77 | 866.19 | 359.36 | 415.86 |
| 1 | 64 | 957.14 | 912.38 | 574.15 | 713.20 |
| 1 | 128 | 1,006.44 | 1,054.43 | 997.38 | 1,300.50 |
| 1 | 256 | 1,156.74 | 1,151.87 | 1,825.35 | 2,148.52 |
| 1 | 512 | 1,460.09 | 1,499.52 | 3,464.26 | 3,847.40 |
| 1 | 1,024 | 2,028.60 | 2,063.33 | 6,638.62 | 7,295.04 |
| 1 | 2,048 | 3,225.57 | 3,402.08 | 13,100.30 | 14,155.08 |

Complete seven-metric records for all 64 configurations are in
[summary.csv](../bench_results/sha256-ecdsa-compare-shared-kernels-i4-i11/summary.csv); all 256 warmup/measured samples
are in [samples.csv](../bench_results/sha256-ecdsa-compare-shared-kernels-i4-i11/samples.csv). E2E above is derived from the
raw per-run `witness_to_proof_ms` and `verify_ms` fields.

## Security settings and reproducibility

F2Z uses its **100-bit economic security target**, including proof of work.
Measured economic bounds are 100.02–100.31 bits; its
reported statistical lower bounds are 98.13–98.66
bits. Spartan uses **nominal 128-bit group security**, with discrete-log and
Fiat–Shamir assumptions and no equivalent statistical bound reported here.
These settings use different security accounting. Neither benchmark promises
zero knowledge. Repeating one fixture repeats the same F2Z grinding task;
these samples do not describe a distribution over different messages.

- CPU: AMD Ryzen 9 9950X3D 16-Core Processor; 32 logical CPUs.
- OS: `Linux-6.17.0-41-generic-x86_64-with-glibc2.42`.
- Compiler: `rustc 1.97.1 (8bab26f4f 2026-07-14)`.
- F2Z repository base commit: `ec8c9f6504bddcb96d3fb2dd840a21014f3b3397`.
- Working-tree diff SHA-256: `2378b5953e2b75be86a0293d2f3433d2f66fbc832a63713707adb926297f7bf6`; source and
  working-tree status are retained in the campaign artifacts.
- Spartan backend commit: `3b94130453838550a1086ee75c5d280f5c25df3d`; every sample records this revision.
- Benchmark binary SHA-256: `59926a8c72c7c606900bb4f31b3dc6ac66be4e9797c6e4dc2c910cfd5f991e2b`. The report generator
  checked the binary against this campaign manifest.
- Rust flags: `-C target-cpu=native`; fixed Rayon pools of 1 or 32 threads, fresh
  worker processes for each configuration.

The [manifest](../bench_results/sha256-ecdsa-compare-shared-kernels-i4-i11/manifest.json),
[requested cases](../bench_results/sha256-ecdsa-compare-shared-kernels-i4-i11/requested_cases.json),
[comparison checks](../bench_results/sha256-ecdsa-compare-shared-kernels-i4-i11/comparison.json), per-worker raw JSON,
stdout/stderr and RSS files preserve the original measurements. Generating
this report validates all 64 cases, all 256 verified samples, common fixtures,
timing partitions, CSV medians, a single backend revision and the manifested
binary hash. It does not modify measurement files.
