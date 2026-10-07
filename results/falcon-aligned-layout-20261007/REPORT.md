# Aligned Falcon layouts

480 verified proofs. Medians of five measured trials after one warm-up; matched inputs, both security targets.

The implementation is complete for both paths. Algebraic Falcon packs each signature into 32,768 bits (previously 65,536) and uses factored signed-coefficient binding. Full Falcon uses aligned coefficient blocks within its existing source size and now authenticates internal, trailing, and inactive padding as zero. Its exact CT signature-byte binding is preserved. Transcript/layout versions changed; proofs must be regenerated.

The algebraic prover is faster in all 20 configurations here (22.2–64.9% lower total time). At 1,024 signatures and the 128-bit target, one-thread total prover time changes from 2,411.60 to 877.20 ms; stored payload changes from 461.81 to 355.75 KiB, and packed source size changes from 8 to 4 MiB. Batches of at most eight retain the existing PCS minimum allocation.

**Full Falcon does not show an overall speedup in this experiment.** Total times increase by 0.2–15.4%. Separate traced runs show that about 899 ms of the largest one-thread difference occurs in the shared PCS opening, with about 897 ms in its grinding stage. Full binding itself has no consistent speedup. These fixed-seed measurements include the changed transcript's grinding work and the added padding enforcement; do not describe them as a pure layout-cost comparison. See [performance analysis](PERFORMANCE.md) and [validation](VALIDATION.md).

The correctness suite passes 186 distinct tests plus one-thread repeats. All 480 main benchmark proofs and 96 separate diagnostic proofs verify. No competitor comparison is claimed: the algebraic path measured here is Falcon-1024, while the supplied LaZer/Orthus results use Falcon-512.

| Prover | Batch | Bits | Threads | Total before / after (ms) | Reduction | Verify before / after (ms) | Payload before / after (KiB) |
|---|---:|---:|---:|---:|---:|---:|---:|
| algebraic | 32 | 100 | 1 | 81.72 / 34.10 | 58.3% | 9.12 / 8.54 | 131.43 / 116.29 |
| algebraic | 32 | 100 | 2 | 45.39 / 20.87 | 54.0% | 6.83 / 6.24 | 131.43 / 116.29 |
| algebraic | 32 | 100 | 4 | 27.97 / 14.87 | 46.8% | 5.73 / 5.12 | 131.43 / 116.29 |
| algebraic | 32 | 100 | 8 | 18.67 / 11.80 | 36.8% | 5.23 / 4.63 | 131.43 / 116.29 |
| algebraic | 32 | 100 | 16 | 17.43 / 13.32 | 23.6% | 5.21 / 4.48 | 131.43 / 116.29 |
| algebraic | 32 | 128 | 1 | 101.21 / 47.41 | 53.2% | 9.72 / 8.94 | 152.74 / 134.96 |
| algebraic | 32 | 128 | 2 | 54.94 / 28.30 | 48.5% | 7.44 / 6.64 | 152.74 / 134.96 |
| algebraic | 32 | 128 | 4 | 33.36 / 19.47 | 41.6% | 6.33 / 5.55 | 152.74 / 134.96 |
| algebraic | 32 | 128 | 8 | 22.06 / 14.94 | 32.3% | 5.89 / 5.01 | 152.74 / 134.96 |
| algebraic | 32 | 128 | 16 | 20.60 / 16.02 | 22.2% | 5.81 / 4.94 | 152.74 / 134.96 |
| algebraic | 1024 | 100 | 1 | 2340.35 / 821.29 | 64.9% | 212.87 / 209.98 | 409.15 / 308.71 |
| algebraic | 1024 | 100 | 2 | 1293.12 / 502.53 | 61.1% | 136.76 / 134.87 | 409.15 / 308.71 |
| algebraic | 1024 | 100 | 4 | 766.59 / 345.07 | 55.0% | 99.66 / 97.56 | 409.15 / 308.71 |
| algebraic | 1024 | 100 | 8 | 505.71 / 265.31 | 47.5% | 81.32 / 80.57 | 409.15 / 308.71 |
| algebraic | 1024 | 100 | 16 | 392.12 / 243.16 | 38.0% | 71.60 / 69.93 | 409.15 / 308.71 |
| algebraic | 1024 | 128 | 1 | 2411.60 / 877.20 | 63.6% | 212.20 / 210.28 | 461.81 / 355.75 |
| algebraic | 1024 | 128 | 2 | 1323.52 / 532.84 | 59.7% | 138.18 / 134.97 | 461.81 / 355.75 |
| algebraic | 1024 | 128 | 4 | 788.43 / 362.45 | 54.0% | 99.77 / 98.74 | 461.81 / 355.75 |
| algebraic | 1024 | 128 | 8 | 516.73 / 274.63 | 46.9% | 82.70 / 81.20 | 461.81 / 355.75 |
| algebraic | 1024 | 128 | 16 | 403.23 / 250.76 | 37.8% | 73.89 / 71.69 | 461.81 / 355.75 |
| full | 32 | 100 | 1 | 151.22 / 155.67 | -2.9% | 23.86 / 24.72 | 254.15 / 253.65 |
| full | 32 | 100 | 2 | 84.11 / 86.36 | -2.7% | 18.11 / 18.78 | 254.15 / 253.65 |
| full | 32 | 100 | 4 | 51.70 / 53.22 | -2.9% | 15.79 / 16.35 | 254.15 / 253.65 |
| full | 32 | 100 | 8 | 36.65 / 37.37 | -2.0% | 14.51 / 14.95 | 254.15 / 253.65 |
| full | 32 | 100 | 16 | 38.39 / 41.09 | -7.0% | 14.27 / 15.01 | 254.15 / 253.65 |
| full | 32 | 128 | 1 | 509.95 / 514.03 | -0.8% | 25.43 / 26.13 | 314.71 / 312.11 |
| full | 32 | 128 | 2 | 266.81 / 269.42 | -1.0% | 19.72 / 20.15 | 314.71 / 312.11 |
| full | 32 | 128 | 4 | 147.15 / 148.20 | -0.7% | 17.18 / 17.69 | 314.71 / 312.11 |
| full | 32 | 128 | 8 | 87.70 / 88.91 | -1.4% | 16.19 / 16.67 | 314.71 / 312.11 |
| full | 32 | 128 | 16 | 69.18 / 70.38 | -1.7% | 15.72 / 16.63 | 314.71 / 312.11 |
| full | 1024 | 100 | 1 | 4535.26 / 4547.67 | -0.3% | 429.18 / 422.56 | 637.33 / 640.20 |
| full | 1024 | 100 | 2 | 2429.35 / 2462.76 | -1.4% | 246.38 / 254.85 | 637.33 / 640.20 |
| full | 1024 | 100 | 4 | 1388.81 / 1404.03 | -1.1% | 153.34 / 157.16 | 637.33 / 640.20 |
| full | 1024 | 100 | 8 | 880.99 / 885.27 | -0.5% | 94.44 / 94.80 | 637.33 / 640.20 |
| full | 1024 | 100 | 16 | 687.54 / 688.87 | -0.2% | 70.01 / 70.65 | 637.33 / 640.20 |
| full | 1024 | 128 | 1 | 6431.05 / 7418.44 | -15.4% | 439.61 / 451.06 | 795.46 / 794.62 |
| full | 1024 | 128 | 2 | 3412.96 / 3907.44 | -14.5% | 255.80 / 256.46 | 795.46 / 794.62 |
| full | 1024 | 128 | 4 | 1931.14 / 2171.42 | -12.4% | 159.70 / 159.49 | 795.46 / 794.62 |
| full | 1024 | 128 | 8 | 1193.69 / 1316.71 | -10.3% | 98.08 / 98.54 | 795.46 / 794.62 |
| full | 1024 | 128 | 16 | 889.80 / 956.76 | -7.5% | 74.02 / 74.43 | 795.46 / 794.62 |

The algebraic source slot falls from 65,536 to 32,768 bits. Full arithmetic retains 131,072 bits plus its existing Keccak sources. Protocol layouts and challenges change; proof bytes need not match the baseline. Timings include grinding, whose nonce work can vary across protocol versions. Full payload excludes the 32-byte source root; algebraic payload includes it. The full-prover change includes authenticated padding enforcement and its increased link-grinding budget as well as aligned coefficient binding.

Detailed phase timings, source bytes, process RSS, raw trials, source hashes, and commands are included alongside this report. Process RSS includes input setup and persistent caches.
