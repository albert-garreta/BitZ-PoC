# SHA-256 / P-256 comparison: every exponent 4 through 11

**Historical implementation:** these results use Spartan2 commit `bf99f4f8`,
before the non-ZK adapter was refactored to reuse the existing optimized
NeutronNova and sumcheck kernels. They are not measurements of the current
shared-kernel implementation.

See the [current shared-kernel results](sha256-ecdsa-shared-kernels-results.md)
for the replacement measurements.

All **64 configurations** completed, covering **16, 32, 64, 128, 256, 512,
1,024 and 2,048 SHA-256 compressions**. All **256 proofs**, including warmups,
were encoded, decoded and verified. Every chain includes one standard padding
compression and one P-256 ECDSA verification. Fixtures matched across all
methods and chunkings at each exponent.

Each configuration has one warmup and three measured repetitions on a single
fixture (`seed=0`). Times below are medians in milliseconds. This campaign uses
the same binary as the [initial measurements](sha256-ecdsa-comparison-results.md).
The benchmarked Rust code was unchanged; the reporting script now derives the
requested PIOP/IOP columns and exports every original sample.

## What each metric measures

| Requested metric | Column and boundary |
| --- | --- |
| Commit time | `commit_ms`: source commitment for F2Z; all SHA/core commitments for Spartan |
| Witness generation | `witness_ms`: fresh witness synthesis, including chaining states and signature hints |
| PIOP time | `piop_ms = protocol_ms - opening_ms`: all non-opening protocol work, including projection/preparation, matrix work, folding, sumchecks and transcript/boundary overhead |
| IOP time | `iop_ms = opening_ms`: the complete F2Z virtual opening path, or Spartan's combined witness/blind construction and direct Hyrax opening |
| Verifier time | `verify_ms`: complete application verification, including chunk links, padding and public input binding |
| Proof size | `proof_material_bytes`: complete encoded proof material, including commitments and proof-carried inputs; the 129-byte expected statement is additional |
| Max peak memory | `peak_rss_bytes`: GNU time's whole-worker maximum, including setup, warmup, measured proofs and verification |

PIOP includes preparation outside the named sumcheck scopes; it is not only
the sumcheck kernel time. IOP/PCS is an accounting label for the opening stage;
Spartan does not add a separate oracle protocol here. F2Z's opening-input
preparation remains in the non-opening category. Per sample,
`commit + PIOP + IOP/PCS = prove`. The subtraction happens before taking
medians, so medians of columns need not add up to the median total.

Peak memory is one measurement per worker, not per proof. It is repeated on
that worker's rows in `samples.csv`. Setup and codec times remain separate in
the CSV. Fixture generation/signing occurs outside the timers.

## Comparison settings

**Security assumptions differ.** F2Z uses the 100-bit economic target with
proof of work; measured economic bounds span 100.02–100.31
bits and statistical lower bounds span 98.13–98.66
bits. Spartan records nominal 128-bit group security under DLOG and
Fiat–Shamir assumptions, without an equivalent reported statistical bound.
This is a comparison of these configurations, not equal statistical security.

Spartan is the new non-ZK NeutronNova/Spartan implementation in the local fork,
using its matrix and direct Hyrax infrastructure. It is not a reproduction of
the Vega paper's optimized ZK implementation or published timings. Neither
method promises zero knowledge in this benchmark.

Two Spartan chunking policies are measured explicitly:

- **B=1:** `r=0,c=i`, one compression per SHA instance.
- **B≤16:** eight SHA chunks through 128 compressions, then at most 16
  compressions per chunk: `r=min(4,i-3), c=i-r`.

| i | Total compressions | Message bytes | B=1: r:c | B≤16: r:c |
| ---: | ---: | ---: | --- | --- |
| 4 | 16 | 960 | 0:4 | 1:3 |
| 5 | 32 | 1,984 | 0:5 | 2:3 |
| 6 | 64 | 4,032 | 0:6 | 3:3 |
| 7 | 128 | 8,128 | 0:7 | 4:3 |
| 8 | 256 | 16,320 | 0:8 | 4:4 |
| 9 | 512 | 32,704 | 0:9 | 4:5 |
| 10 | 1,024 | 65,472 | 0:10 | 4:6 |
| 11 | 2,048 | 131,008 | 0:11 | 4:7 |

F2Z uses only total i and is measured once per mode/thread count. The two
Spartan policies are not an exhaustive search for the optimal r:c split.
Repeated F2Z trials at one seed repeat the same grinding task; these timings
describe one fixture per length, rather than a distribution over messages.

## Prover totals for comparison

These totals include commitment, PIOP and IOP/PCS, excluding witness generation.
The complete seven-metric tables follow.

### 1 thread

| Compressions | F2Z Split | F2Z AllRows | Spartan B=1 | Spartan B≤16 |
| ---: | ---: | ---: | ---: | ---: |
| 16 | 899.63 | 848.75 | 286.92 | 304.84 |
| 32 | 908.64 | 860.89 | 430.61 | 495.50 |
| 64 | 945.15 | 924.27 | 688.82 | 855.54 |
| 128 | 1014.94 | 1041.81 | 1239.97 | 1614.49 |
| 256 | 1170.02 | 1147.42 | 2315.54 | 2722.11 |
| 512 | 1451.49 | 1491.45 | 4412.82 | 4887.97 |
| 1,024 | 2030.22 | 2084.91 | 8674.86 | 9241.29 |
| 2,048 | 3266.72 | 3402.65 | 16977.19 | 17815.21 |

### 32 threads

| Compressions | F2Z Split | F2Z AllRows | Spartan B=1 | Spartan B≤16 |
| ---: | ---: | ---: | ---: | ---: |
| 16 | 778.87 | 726.47 | 60.02 | 94.72 |
| 32 | 777.55 | 720.38 | 75.31 | 188.78 |
| 64 | 781.07 | 748.26 | 102.63 | 363.31 |
| 128 | 779.13 | 787.23 | 165.27 | 724.09 |
| 256 | 795.14 | 774.02 | 271.40 | 863.44 |
| 512 | 826.41 | 835.31 | 479.27 | 1091.00 |
| 1,024 | 846.56 | 852.25 | 876.05 | 1597.82 |
| 2,048 | 948.93 | 1056.18 | 1646.57 | 2474.55 |

## All seven metrics

Times are **ms**, proof material is **KiB**, and peak resident memory is **MiB**.

### F2Z Split, 1 thread

| Compressions | Commit | Witness | PIOP | IOP/PCS | Verify | Proof KiB | Peak MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 | 0.75 | 14.98 | 822.72 | 75.70 | 734.19 | 144.93 | 1221.70 |
| 32 | 0.86 | 17.17 | 825.28 | 81.79 | 728.73 | 146.53 | 1224.34 |
| 64 | 0.95 | 22.47 | 852.72 | 90.57 | 741.19 | 152.26 | 1219.64 |
| 128 | 1.18 | 32.77 | 897.47 | 113.62 | 730.65 | 162.45 | 1222.34 |
| 256 | 1.70 | 54.21 | 1003.69 | 165.07 | 737.01 | 176.37 | 1245.79 |
| 512 | 3.30 | 97.05 | 1193.89 | 256.30 | 760.16 | 221.29 | 1279.90 |
| 1,024 | 3.65 | 182.47 | 1579.76 | 446.81 | 764.53 | 219.77 | 1324.04 |
| 2,048 | 3.34 | 352.82 | 2418.62 | 843.98 | 829.78 | 278.49 | 1505.86 |

### F2Z AllRows, 1 thread

| Compressions | Commit | Witness | PIOP | IOP/PCS | Verify | Proof KiB | Peak MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 | 0.99 | 15.03 | 771.86 | 76.10 | 687.36 | 143.90 | 1221.34 |
| 32 | 1.05 | 17.55 | 776.05 | 81.55 | 683.48 | 145.87 | 1224.07 |
| 64 | 1.21 | 22.91 | 830.78 | 92.48 | 714.45 | 152.26 | 1219.80 |
| 128 | 0.76 | 32.33 | 925.70 | 114.08 | 754.09 | 162.26 | 1222.62 |
| 256 | 1.69 | 54.05 | 982.63 | 162.37 | 723.53 | 176.34 | 1253.10 |
| 512 | 3.05 | 96.96 | 1227.31 | 261.16 | 754.47 | 221.60 | 1279.10 |
| 1,024 | 3.49 | 182.48 | 1635.45 | 446.62 | 735.99 | 221.89 | 1307.67 |
| 2,048 | 3.33 | 348.73 | 2559.81 | 842.71 | 856.00 | 277.55 | 1561.60 |

### Spartan B=1, 1 thread

| Compressions | Commit | Witness | PIOP | IOP/PCS | Verify | Proof KiB | Peak MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 | 155.70 | 51.61 | 130.67 | 0.61 | 26.10 | 97.88 | 340.94 |
| 32 | 206.23 | 96.21 | 222.59 | 0.61 | 29.26 | 122.51 | 428.20 |
| 64 | 302.70 | 184.28 | 383.87 | 0.61 | 35.35 | 171.63 | 604.84 |
| 128 | 499.13 | 373.24 | 739.16 | 0.61 | 43.29 | 269.76 | 956.61 |
| 256 | 891.26 | 720.37 | 1424.54 | 0.61 | 58.98 | 465.88 | 1662.18 |
| 512 | 1678.50 | 1411.84 | 2733.50 | 0.60 | 87.63 | 858.01 | 3083.31 |
| 1,024 | 3264.50 | 2841.02 | 5403.87 | 0.53 | 140.09 | 1642.13 | 5902.71 |
| 2,048 | 6385.01 | 5634.93 | 10595.31 | 0.49 | 237.29 | 3210.26 | 11540.96 |

### Spartan B≤16, 1 thread

| Compressions | Commit | Witness | PIOP | IOP/PCS | Verify | Proof KiB | Peak MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 | 155.78 | 48.77 | 148.06 | 1.20 | 32.23 | 94.59 | 382.57 |
| 32 | 204.54 | 99.92 | 288.37 | 2.71 | 49.67 | 112.30 | 551.35 |
| 64 | 302.27 | 183.62 | 547.83 | 5.11 | 83.08 | 147.30 | 909.05 |
| 128 | 495.71 | 375.14 | 1109.54 | 9.53 | 149.41 | 216.87 | 1578.03 |
| 256 | 889.68 | 742.03 | 1821.18 | 10.35 | 178.04 | 351.12 | 2282.18 |
| 512 | 1669.54 | 1531.28 | 3209.05 | 10.09 | 221.88 | 619.49 | 3697.08 |
| 1,024 | 3220.16 | 2935.49 | 6012.43 | 9.65 | 302.93 | 1156.12 | 6522.80 |
| 2,048 | 6321.44 | 5958.48 | 11481.57 | 9.36 | 440.60 | 2229.24 | 12146.62 |

### F2Z Split, 32 threads

| Compressions | Commit | Witness | PIOP | IOP/PCS | Verify | Proof KiB | Peak MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 | 0.76 | 4.18 | 749.25 | 28.98 | 736.60 | 144.93 | 1228.42 |
| 32 | 0.79 | 4.55 | 746.17 | 30.92 | 731.32 | 146.53 | 1228.01 |
| 64 | 0.77 | 4.88 | 749.33 | 31.00 | 737.30 | 152.26 | 1228.14 |
| 128 | 0.75 | 5.17 | 741.63 | 36.75 | 722.39 | 162.45 | 1224.68 |
| 256 | 0.90 | 6.48 | 750.58 | 45.18 | 728.85 | 176.37 | 1249.76 |
| 512 | 0.94 | 8.10 | 764.68 | 59.47 | 738.56 | 221.29 | 1283.62 |
| 1,024 | 0.98 | 12.25 | 767.86 | 77.75 | 715.96 | 219.77 | 1342.92 |
| 2,048 | 1.69 | 20.53 | 824.29 | 124.56 | 724.44 | 278.49 | 1562.08 |

### F2Z AllRows, 32 threads

| Compressions | Commit | Witness | PIOP | IOP/PCS | Verify | Proof KiB | Peak MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 | 0.77 | 4.40 | 695.79 | 29.01 | 685.23 | 143.90 | 1225.74 |
| 32 | 0.77 | 4.47 | 688.08 | 31.01 | 680.45 | 145.87 | 1229.17 |
| 64 | 0.76 | 4.66 | 716.34 | 31.23 | 702.78 | 152.26 | 1227.79 |
| 128 | 0.73 | 5.16 | 750.13 | 35.64 | 733.70 | 162.26 | 1228.12 |
| 256 | 0.88 | 6.20 | 727.77 | 45.30 | 703.81 | 176.34 | 1258.85 |
| 512 | 0.97 | 8.24 | 775.14 | 60.61 | 725.39 | 221.60 | 1284.51 |
| 1,024 | 1.10 | 12.28 | 772.09 | 77.88 | 682.28 | 221.89 | 1386.34 |
| 2,048 | 1.77 | 20.43 | 930.88 | 123.62 | 746.28 | 277.55 | 1619.70 |

### Spartan B=1, 32 threads

| Compressions | Commit | Witness | PIOP | IOP/PCS | Verify | Proof KiB | Peak MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 | 15.78 | 10.57 | 43.93 | 0.29 | 17.61 | 97.88 | 462.93 |
| 32 | 19.90 | 12.31 | 53.71 | 0.29 | 18.15 | 122.51 | 551.16 |
| 64 | 29.54 | 16.99 | 72.76 | 0.31 | 19.73 | 171.63 | 757.18 |
| 128 | 38.58 | 26.41 | 126.61 | 0.33 | 21.55 | 269.76 | 1047.98 |
| 256 | 55.89 | 43.69 | 214.42 | 0.30 | 25.56 | 465.88 | 1771.09 |
| 512 | 103.89 | 79.57 | 376.60 | 0.30 | 32.82 | 858.01 | 3169.75 |
| 1,024 | 192.74 | 154.19 | 683.81 | 0.30 | 46.14 | 1642.13 | 6001.29 |
| 2,048 | 366.39 | 298.94 | 1280.28 | 0.30 | 74.37 | 3210.26 | 11663.69 |

### Spartan B≤16, 32 threads

| Compressions | Commit | Witness | PIOP | IOP/PCS | Verify | Proof KiB | Peak MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 | 17.20 | 12.40 | 75.69 | 0.60 | 19.84 | 94.59 | 665.47 |
| 32 | 19.46 | 18.78 | 169.34 | 1.06 | 26.15 | 112.30 | 969.81 |
| 64 | 28.09 | 32.05 | 334.46 | 1.74 | 35.84 | 147.30 | 1750.64 |
| 128 | 35.27 | 60.67 | 681.34 | 3.35 | 55.83 | 216.87 | 3467.78 |
| 256 | 55.51 | 69.69 | 803.80 | 3.30 | 59.66 | 351.12 | 3453.71 |
| 512 | 99.54 | 99.48 | 988.56 | 3.33 | 66.34 | 619.49 | 3715.79 |
| 1,024 | 186.55 | 163.50 | 1409.08 | 3.31 | 79.89 | 1156.12 | 6549.05 |
| 2,048 | 363.61 | 319.04 | 2110.42 | 3.22 | 105.26 | 2229.24 | 12421.10 |

## Reproduction and artifacts

- CPU: AMD Ryzen 9 9950X3D 16-Core Processor; 32 logical CPUs.
- OS: `Linux-6.17.0-41-generic-x86_64-with-glibc2.42`.
- Compiler: `rustc 1.97.1 (8bab26f4f 2026-07-14)`.
- Root base revision: `ec8c9f6504bddcb96d3fb2dd840a21014f3b3397`, with comparison changes uncommitted.
- Backend revision: `bf99f4f828131afd153e239a4ea768ca5062601e`.
- Binary SHA-256: `f13b83c12c42c500af2d3d14ad945f5a72d149fca5c7b4035be27ac07cd9c443`.
- Native CPU release build, LTO, one codegen unit; fixed Rayon pools of 1 or
  32 threads; fresh worker processes; no CPU affinity.
- Limits: 48 GiB virtual address space and 900 seconds per case.

```sh
python3 scripts/run_sha256_ecdsa_compare.py \
  --output bench_results/sha256-ecdsa-compare-i4-i11 \
  --spartan-splits 0:4 1:3 0:5 2:3 0:6 3:3 0:7 4:3 \
           0:8 4:4 0:9 4:5 0:10 4:6 0:11 4:7 \
  --targets 100 --threads 1 32 --reps 3 --seeds 0 \
  --timeout 900 --memory-gib 48
```

This run reused the existing binary with `--binary` and
`CARGO_HOME=/tmp/bitz-sha-cargo-home`; its backend Git object was available
locally. The same backend commit has since been published on the Spartan2
fork's `f2z-benching` branch.
The [methodology guide](sha256-ecdsa-comparison.md) documents the APIs and
timing boundaries.

Raw artifacts are in `bench_results/sha256-ecdsa-compare-i4-i11/`:

- `summary.csv`: 64 case rows, with median timings and per-worker peak RSS.
- `samples.csv`: 256 warmup/measured rows, with derived PIOP/IOP columns and
  source-file references; raw worker records remain unchanged.
- `*.result.json`, `*.stdout`, `*.stderr`: original measurements and logs.
- `manifest.json`, `requested_cases.json`, `comparison.json`: build, workload
  and fixture-match provenance.
- `source/`: the worker and runner used for execution; `analysis/`: the
  reporting script and metadata used to add the requested metric columns.

All earlier comparison directories also received `summary.csv` and
`samples.csv` with the seven metrics. They remain separate campaigns and are
not pooled with this sweep. The runner's five tests pass, including the check
that derived PIOP medians use per-sample differences and retain warmups/raw data.
