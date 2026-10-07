# Falcon power-basis benchmarks — 2026-10-07

1,024 distinct original-Falcon signatures per proof. Ryzen 9 9950X3D (`will`); native release build (`-C target-cpu=native`, fat LTO, one codegen unit). Medians of five measured runs after one warm-up, seed 42. Prover times include checked witness preparation, packing, commitment, and proving. Input generation, external hash-to-point preparation for the algebraic statement, and reusable parameter setup are excluded. Full Falcon proves SHAKE-256 and HashToPoint; algebraic Falcon proves the public-h/public-t ring equation and integer norm.

Implementation details and coordinate/bound contracts are in [IMPLEMENTATION.md](IMPLEMENTATION.md).

## Baseline identity

Existing paths use commit `5c051cf767ab3b69f23ebaa036b6e6e3c6d1f726`. That revision has no algebraic Falcon-512 frontend. Its baseline is the new degree-512 port **before** the power-basis optimization, using the same ordinary extension multiplications as the old degree-1024 path. `port512.patch` records that port; a hard-coded padding-count test expectation was subsequently generalized. Binary checksums and source hashes are recorded for each build.

The optimized algebraic prover retains canonical coordinates in the challenge power basis through carries and projection. Both full and algebraic Falcon share the field backend in `vendor/field`. Full Falcon retains its fixed-basis tensor lift; these measurements do not implement a power-basis lift for full Falcon. The NTT proposal is also separate.

## Results

All times below are milliseconds. Each row uses the same degree, target, inputs, thread count, and affinity before/after. CPUs are pinned to logical CPU IDs 0 through threads-1; auxiliary workers and the main thread are pinned to CPU 0. There were no simultaneous benchmark, build, or test jobs during measured runs. Both algebraic targets use extension degree 11; full Falcon uses degree 9 at 100 bits and degree 11 at 128 bits.

### Algebraic Falcon-512, 100-bit target

| Threads | Baseline prover | Updated prover | Ratio | Baseline verify | Updated verify | Payload KiB before → after |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 407.23 | 356.89 | 1.14× | 103.03 | 54.29 | 219.56 → 221.74 |
| 2 | 247.35 | 221.86 | 1.11× | 65.86 | 40.62 | 219.56 → 221.74 |
| 4 | 169.48 | 156.19 | 1.09× | 47.62 | 35.04 | 219.56 → 221.74 |
| 8 | 129.64 | 122.61 | 1.06× | 37.81 | 31.44 | 219.56 → 221.74 |
| 16 | 122.60 | 120.04 | 1.02× | 34.38 | 31.51 | 219.56 → 221.74 |

### Algebraic Falcon-1024, 100-bit target

| Threads | Baseline prover | Updated prover | Ratio | Baseline verify | Updated verify | Payload KiB before → after |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 821.18 | 722.65 | 1.14× | 210.21 | 112.80 | 308.71 → 307.39 |
| 2 | 500.21 | 450.76 | 1.11× | 134.55 | 85.75 | 308.71 → 307.39 |
| 4 | 345.19 | 318.28 | 1.08× | 98.26 | 73.75 | 308.71 → 307.39 |
| 8 | 263.85 | 250.00 | 1.06× | 79.48 | 66.65 | 308.71 → 307.39 |
| 16 | 243.28 | 235.58 | 1.03× | 70.14 | 64.45 | 308.71 → 307.39 |

### Full Falcon-512, 100-bit target

| Threads | Baseline prover | Updated prover | Ratio | Baseline verify | Updated verify | Payload KiB before → after |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2421.79 | 2427.84 | 1.00× | 223.88 | 233.46 | 482.92 → 482.92 |
| 2 | 1288.78 | 1290.41 | 1.00× | 134.53 | 134.79 | 482.92 → 482.92 |
| 4 | 721.19 | 722.38 | 1.00× | 78.23 | 78.17 | 482.92 → 482.92 |
| 8 | 449.22 | 446.94 | 1.01× | 53.77 | 53.99 | 482.92 → 482.92 |
| 16 | 355.82 | 357.06 | 1.00× | 41.03 | 41.04 | 482.92 → 482.92 |

### Full Falcon-1024, 100-bit target

| Threads | Baseline prover | Updated prover | Ratio | Baseline verify | Updated verify | Payload KiB before → after |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 4553.04 | 4558.49 | 1.00× | 422.57 | 422.30 | 640.20 → 640.20 |
| 2 | 2458.32 | 2459.53 | 1.00× | 254.44 | 253.71 | 640.20 → 640.20 |
| 4 | 1400.31 | 1397.22 | 1.00× | 155.53 | 155.70 | 640.20 → 640.20 |
| 8 | 884.40 | 884.54 | 1.00× | 94.66 | 94.42 | 640.20 → 640.20 |
| 16 | 686.40 | 688.54 | 1.00× | 70.78 | 70.65 | 640.20 → 640.20 |

### Algebraic Falcon-512, 128-bit target

| Threads | Baseline prover | Updated prover | Ratio | Baseline verify | Updated verify | Payload KiB before → after |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 453.52 | 402.64 | 1.13× | 103.92 | 54.65 | 256.93 → 258.28 |
| 2 | 271.06 | 244.52 | 1.11× | 66.53 | 41.54 | 256.93 → 258.28 |
| 4 | 182.34 | 169.25 | 1.08× | 48.60 | 35.81 | 256.93 → 258.28 |
| 8 | 137.32 | 130.28 | 1.05× | 38.45 | 33.05 | 256.93 → 258.28 |
| 16 | 129.04 | 125.35 | 1.03× | 35.09 | 32.31 | 256.93 → 258.28 |

### Algebraic Falcon-1024, 128-bit target

| Threads | Baseline prover | Updated prover | Ratio | Baseline verify | Updated verify | Payload KiB before → after |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 878.11 | 795.50 | 1.10× | 211.15 | 114.09 | 355.75 → 353.10 |
| 2 | 531.58 | 488.36 | 1.09× | 135.79 | 87.00 | 355.75 → 353.10 |
| 4 | 360.80 | 338.56 | 1.07× | 98.94 | 74.66 | 355.75 → 353.10 |
| 8 | 272.36 | 261.46 | 1.04× | 80.09 | 67.93 | 355.75 → 353.10 |
| 16 | 250.37 | 244.73 | 1.02× | 71.00 | 65.54 | 355.75 → 353.10 |

### Full Falcon-512, 128-bit target

| Threads | Baseline prover | Updated prover | Ratio | Baseline verify | Updated verify | Payload KiB before → after |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 3680.08 | 3681.99 | 1.00× | 235.40 | 235.89 | 600.25 → 600.25 |
| 2 | 1923.83 | 1923.34 | 1.00× | 129.87 | 130.49 | 600.25 → 600.25 |
| 4 | 1058.49 | 1056.39 | 1.00× | 80.45 | 80.63 | 600.25 → 600.25 |
| 8 | 638.51 | 640.88 | 1.00× | 56.22 | 56.24 | 600.25 → 600.25 |
| 16 | 471.40 | 474.47 | 0.99× | 43.67 | 43.44 | 600.25 → 600.25 |

### Full Falcon-1024, 128-bit target

| Threads | Baseline prover | Updated prover | Ratio | Baseline verify | Updated verify | Payload KiB before → after |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 7405.00 | 7416.30 | 1.00× | 435.24 | 450.45 | 794.62 → 794.62 |
| 2 | 3896.56 | 3892.33 | 1.00× | 256.58 | 255.93 | 794.62 → 794.62 |
| 4 | 2167.62 | 2168.43 | 1.00× | 155.27 | 157.70 | 794.62 → 794.62 |
| 8 | 1317.43 | 1316.95 | 1.00× | 98.35 | 97.89 | 794.62 → 794.62 |
| 16 | 954.46 | 957.54 | 1.00× | 74.05 | 74.47 | 794.62 → 794.62 |

## Interpretation

Ratios are observed whole-prover measurements for this fixed input seed. The algebraic protocol has a new transcript domain and different carry values, which also change grinding challenges. Repeating the same input primarily measures timing noise, not the distribution of grinding work across seeds. Do not attribute every whole-prover difference to weight generation. Full Falcon has no algorithmic lift change and should be interpreted as a backend-move regression check.

Proof message schemas, source sizes, and commitment counts are unchanged by the basis optimization. Actual serialized payload sizes can vary with transcript-dependent Merkle multiproof overlap. The algebraic payload changes in these runs are confined to PCS openings; ring and arithmetic messages have identical sizes. Payload sizes exclude outer transport framing. Algebraic version-2 proofs are incompatible with the new version-3 transcript despite having the same message layout.

## Isolated public field kernel

`vendor/field/examples/q12289_power_basis.rs` generates two weight sequences per signature for 1,024 signatures on one thread. Both paths retain all output vectors. The updated timing includes matrix setup and both start conversions per signature; correctness checks run outside the timer. Trial 0 is warm-up. An initial measurement immediately after compilation showed timing drift for degree 512 and is retained as `kernel-benchmark-initial.jsonl`. The reported run follows an additional untimed complete pass. This isolates field arithmetic and has no commitments, sumchecks, projection, or grinding.

| Degree | Ordinary multiplication ms | Power basis ms | Ratio | Basis setup µs |
|---:|---:|---:|---:|---:|
| 512 | 58.324 | 7.459 | 7.82× | 3.206 |
| 1024 | 116.596 | 14.863 | 7.84× | 3.006 |

## Ring-coordinate stage trace

Instrumented single-thread runs at 128 bits, one warm-up plus three measured proofs, using the existing tracing spans. These are separate from the uninstrumented matrix. Each entry times public evaluation, target formation, and weight-coordinate construction for 1,024 signatures. The optimized span also includes cached-power generation and basis setup.

| Degree | Stage | Baseline ms | Updated ms | Ratio |
|---:|---|---:|---:|---:|
| 512 | prove | 57.801 | 8.830 | 6.55× |
| 512 | verify | 57.900 | 8.900 | 6.51× |
| 1024 | prove | 120.000 | 22.200 | 5.41× |
| 1024 | verify | 121.000 | 22.500 | 5.38× |

## Validation and reproduction

All 480 baseline/updated benchmark proofs verified (40 configurations × 6 proofs × 2 versions), as did 16 additional stage-trace proofs. Raw logs include every trial and proof payload. Input digests match before/after in all configurations; observed payload sizes are reported separately for both versions. Field tests cover registered extension arithmetic, irreducibility, canonical coordinates, recurrence equality, and singular bases. All 175 selected Falcon tests and 3 standalone field tests passed. Four large qualification tests were ignored by the unit-test run; the benchmark matrix separately verifies real 1,024-signature batches. Test results are in `validation/`.

Build: `CARGO_TARGET_DIR=/tmp/falcon-simplify-candidate-target CARGO_BUILD_JOBS=4 RUSTFLAGS="-C target-cpu=native" cargo build --release --locked --offline --features falcon-hybrid --example falcon_algebraic --bench falcon_hybrid`.

Run from the repository root: `python3 results/falcon-power-basis-20261007/bench.py --binary-dir /path/to/preserved/binaries --out /path/to/new/results`. It runs all degrees, targets, and thread counts; use `--kinds algebraic --degrees 512` for the degree-512 algebraic port baseline. Exact executed commands and environment values are in each results directory’s `metadata.json`.

The implementation patch omits unrelated pre-existing workspace changes. `candidate.patch` plus new files listed in its diff reproduce the implementation relative to the baseline revision. Source/binary hash manifests distinguish the baseline revision, unoptimized 512 port, and updated build.
