# Sequential benchmark rerun

**In progress:** 110/152 native cases, 24/24 wide cases, 0/6 SHA+ECDSA cases. Five measured repetitions and one excluded warmup per case. All included cases passed verification.

The previous campaign also ran commands sequentially. This rerun selects one native backend per fresh process, waits 20 seconds between commands, and records system activity approximately every 10 seconds. Each backend process still sweeps sizes 2^15 through 2^22. The BitZ wide CLI also waits 20 seconds between sizes. All provers use eight threads internally.

A global controller lock prevents duplicate campaign controllers. Each command must exit successfully before the next starts. The completed command intervals and cooldowns were checked: [completion.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/completion.json). Exact commands: [COMMANDS.md](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/COMMANDS.md).

Benchmark source, inputs, initial rates, queries, security accounting, and repetition counts are held fixed. All completed native cases have the same recorded configuration and input corpus as their counterparts in the earlier campaign. The process-isolation policy and observation overhead changed, so differences cannot be attributed exclusively to background activity.

## Machine state and interpretation

Sequential execution does not isolate the machine from other applications or OS work. Storage scanning, Spotlight and other application activity were observed. The rerun is not certified as an idle-machine measurement. System swap occupancy alone is not an active paging rate; the telemetry summary uses differences in cumulative VM counters between snapshots.

Telemetry is attached to whole commands, not individual proof intervals. It cannot identify the precise cause of a particular verification spike. `/usr/bin/time -l` resource summaries in the logs include command startup, possible compilation, setup and all trials; they are not per-proof CPU times. Peak RSS from native records comes from a separate verified memory pass; the controller RSS observations have a different boundary.

The raw monitor labels processes outside the command process group as external. SHA workers start in separate process groups; the telemetry summary excludes their exact manifest-recorded executable paths from external CPU totals. The controller RSS guard covers its selected process group, while the SHA runner records each worker’s peak RSS separately.

Observations: [system-observations.jsonl](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/system-observations.jsonl); per-command summaries: [system-observations-summary.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/system-observations-summary.json).

## Binius at 2^22: previous run versus rerun

Times below are five-sample medians in milliseconds. Keep the focused and broader campaigns separate. Complete distributions and per-stage timings remain in [analysis.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/analysis.json).

| Sweep | Rate | Backend | Old witness→proof | New witness→proof | Old verify | New verify | Proof-byte change |
| --- | --- | --- | --- | --- | --- | --- | --- |
| binius-focused | 1/2 | binius64 | 2,799.52 | 2,593.03 | 57.39 | 55.34 | +0.00% |
| binius-focused | 1/2 | binius64-ligerito | 3,757.16 | 3,563.75 | 59.48 | 58.95 | +0.00% |
| binius-focused | 1/4 | binius64 | 3,574.59 | 2,965.78 | 427.67 | 61.37 | +0.00% |
| binius-focused | 1/4 | binius64-ligerito | 3,865.91 | 3,603.24 | 57.56 | 55.32 | +0.00% |
| binius-focused | 1/8 | binius64 | 8,131.45 | 3,467.56 | 463.84 | 67.18 | +0.00% |
| binius-focused | 1/8 | binius64-ligerito | 4,465.84 | 4,663.86 | 54.15 | 78.25 | +0.00% |
| all-provers | 1/2 | binius64 | 2,928.25 | 2,882.76 | 55.77 | 62.93 | +0.00% |
| all-provers | 1/2 | binius64-ligerito | 3,812.12 | 3,716.22 | 58.17 | 61.67 | +0.00% |
| all-provers | 1/4 | binius64 | 2,909.43 | 3,366.64 | 59.53 | 69.15 | +0.00% |
| all-provers | 1/4 | binius64-ligerito | 3,950.09 | 4,373.26 | 51.20 | 59.49 | +0.00% |

## Binius PCS comparison within the rerun

| Sweep | Rate | Witness→proof change | Verifier change | Proof-byte change | RSS change |
| --- | --- | --- | --- | --- | --- |
| binius-focused | 1/2 | +37.4% | +6.5% | -21.2% | -16.6% |
| binius-focused | 1/4 | +21.5% | -9.9% | -29.3% | -14.8% |
| binius-focused | 1/8 | +34.5% | +16.5% | -37.2% | -13.6% |
| all-provers | 1/2 | +28.9% | -2.0% | -21.2% | -8.0% |
| all-provers | 1/4 | +29.9% | -14.0% | -29.3% | -10.4% |

Changes are Ligerito relative to BaseFold at 2^22. They are observations of the configured implementations, which differ in hashing, folding and security accounting as well as PCS. They do not establish an isolated PCS effect or statistical equivalence.

## SHA-256 followed by P-256 ECDSA

One chain of 128 total compressions including padding, 8,128 message bytes, one signature. Standard Binius64 and BitZ Split; the Binius–Ligerito adapter cannot handle the composed circuit. Security-target scopes differ as documented in the earlier audit.

| Rate | Backend | Setup ms | Witness→proof ms | Verify ms | Proof bytes | RSS GiB |
| --- | --- | --- | --- | --- | --- | --- |

## Full-width BitZ multiplication

The CLI proves u32 × u32 → u64. Its prove interval excludes witness construction; its memory figure is tracked heap MiB, not native RSS. Root-inclusive proof bytes add the 32-byte commitment root omitted by the CLI output. The wrapping BitZ backend retains the same full-product relation. Different drivers and inputs prevent paired runtime equivalence claims.

| Rate | Exponent | Witness ms | Prove ms | Verify ms | Root-inclusive proof bytes | Tracked heap MiB |
| --- | --- | --- | --- | --- | --- | --- |
| 1/2 | 15 | 0.05 | 24.36 | 3.92 | 119312 | 29.43 |
| 1/2 | 16 | 0.08 | 37.84 | 3.67 | 138964 | 55.23 |
| 1/2 | 17 | 0.16 | 62.20 | 4.60 | 151904 | 110.00 |
| 1/2 | 18 | 0.30 | 93.80 | 5.33 | 169048 | 212.82 |
| 1/2 | 19 | 0.60 | 158.00 | 5.19 | 187876 | 424.75 |
| 1/2 | 20 | 1.02 | 297.11 | 5.99 | 209664 | 835.19 |
| 1/2 | 21 | 2.21 | 618.93 | 8.69 | 226708 | 1,668.79 |
| 1/2 | 22 | 4.38 | 1,208.75 | 9.07 | 262340 | 3,309.27 |
| 1/4 | 15 | 0.04 | 24.64 | 3.66 | 83360 | 30.68 |
| 1/4 | 16 | 0.09 | 35.98 | 3.18 | 99652 | 57.73 |
| 1/4 | 17 | 0.15 | 54.80 | 4.24 | 107664 | 115.06 |
| 1/4 | 18 | 0.29 | 99.39 | 4.84 | 120104 | 222.82 |
| 1/4 | 19 | 0.56 | 158.76 | 5.37 | 136620 | 444.75 |
| 1/4 | 20 | 1.14 | 306.96 | 5.88 | 154368 | 875.19 |
| 1/4 | 21 | 2.43 | 693.22 | 8.36 | 162964 | 1,748.79 |
| 1/4 | 22 | 3.85 | 1,208.61 | 7.66 | 196708 | 3,469.27 |
| 1/8 | 15 | 0.10 | 28.45 | 4.14 | 70512 | 33.18 |
| 1/8 | 16 | 0.17 | 42.90 | 3.71 | 84788 | 62.79 |
| 1/8 | 17 | 0.15 | 88.44 | 4.28 | 92224 | 125.06 |
| 1/8 | 18 | 0.29 | 101.02 | 4.51 | 102512 | 242.82 |
| 1/8 | 19 | 0.58 | 196.24 | 5.42 | 116180 | 484.75 |
| 1/8 | 20 | 1.17 | 342.32 | 5.38 | 131240 | 955.19 |
| 1/8 | 21 | 2.22 | 592.30 | 7.38 | 139804 | 1,908.79 |
| 1/8 | 22 | 4.65 | 1,311.79 | 7.32 | 171252 | 3,789.27 |

## All native measurements

Times are milliseconds. Setup is separate; phase intervals overlap and must not be summed. Proof sizes include commitment material. Native RSS is a separate full-process memory pass.

| Sweep | Rate | e | Backend | Setup | Witness | Commit | PIOP | Opening | Witness→proof | Verify | Proof bytes | RSS GiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| binius-focused | 1/2 | 15 | binius64 | 152.40 | 0.92 | 1.82 | 44.69 | 5.50 | 52.89 | 2.41 | 324080 | 0.28 |
| binius-focused | 1/2 | 16 | binius64 | 170.01 | 1.80 | 2.54 | 57.99 | 6.06 | 68.75 | 2.66 | 355392 | 0.54 |
| binius-focused | 1/2 | 17 | binius64 | 350.73 | 3.56 | 4.86 | 101.89 | 8.73 | 118.66 | 3.39 | 387888 | 0.92 |
| binius-focused | 1/2 | 18 | binius64 | 732.16 | 7.15 | 8.00 | 169.76 | 11.14 | 195.96 | 4.68 | 431504 | 1.81 |
| binius-focused | 1/2 | 19 | binius64 | 1,523.12 | 14.05 | 15.28 | 307.28 | 16.60 | 353.09 | 11.89 | 472256 | 3.67 |
| binius-focused | 1/2 | 20 | binius64 | 3,054.30 | 28.29 | 31.94 | 732.21 | 31.49 | 823.51 | 20.89 | 518864 | 7.32 |
| binius-focused | 1/2 | 21 | binius64 | 6,515.47 | 56.67 | 65.80 | 1,357.41 | 51.56 | 1,532.08 | 34.45 | 559200 | 14.73 |
| binius-focused | 1/2 | 22 | binius64 | 13,387.97 | 112.23 | 129.11 | 2,264.68 | 87.47 | 2,593.03 | 55.34 | 606800 | 29.48 |
| binius-focused | 1/2 | 15 | binius64-ligerito | 77.65 | 0.85 | 1.53 | 41.23 | 8.32 | 52.32 | 4.35 | 362080 | 0.27 |
| binius-focused | 1/2 | 16 | binius64-ligerito | 172.97 | 1.70 | 2.89 | 59.08 | 11.73 | 75.15 | 4.88 | 373568 | 0.53 |
| binius-focused | 1/2 | 17 | binius64-ligerito | 589.19 | 3.54 | 7.95 | 148.33 | 21.87 | 180.52 | 6.72 | 388352 | 0.90 |
| binius-focused | 1/2 | 18 | binius64-ligerito | 835.25 | 6.97 | 11.32 | 201.60 | 25.94 | 249.33 | 7.09 | 408656 | 1.76 |
| binius-focused | 1/2 | 19 | binius64-ligerito | 1,634.89 | 13.34 | 20.61 | 445.59 | 42.93 | 550.22 | 10.67 | 424816 | 3.35 |
| binius-focused | 1/2 | 20 | binius64-ligerito | 3,272.99 | 27.24 | 44.07 | 846.88 | 70.06 | 980.05 | 17.57 | 438192 | 6.66 |
| binius-focused | 1/2 | 21 | binius64-ligerito | 6,413.18 | 53.38 | 77.31 | 1,598.82 | 139.63 | 1,874.60 | 26.53 | 461872 | 13.30 |
| binius-focused | 1/2 | 22 | binius64-ligerito | 13,637.00 | 107.44 | 163.05 | 2,927.96 | 330.10 | 3,563.75 | 58.95 | 477936 | 24.57 |
| binius-focused | 1/4 | 15 | binius64 | 80.31 | 0.92 | 3.20 | 42.74 | 4.89 | 51.45 | 2.26 | 253008 | 0.29 |
| binius-focused | 1/4 | 16 | binius64 | 170.23 | 1.83 | 4.67 | 64.12 | 6.20 | 76.90 | 2.51 | 272416 | 0.54 |
| binius-focused | 1/4 | 17 | binius64 | 372.66 | 3.61 | 8.27 | 112.84 | 8.05 | 132.88 | 3.23 | 297264 | 0.94 |
| binius-focused | 1/4 | 18 | binius64 | 749.27 | 7.33 | 15.61 | 180.25 | 11.95 | 217.73 | 4.80 | 329248 | 1.86 |
| binius-focused | 1/4 | 19 | binius64 | 1,513.24 | 14.01 | 30.46 | 341.62 | 18.52 | 410.17 | 11.71 | 354192 | 3.74 |
| binius-focused | 1/4 | 20 | binius64 | 3,098.38 | 28.07 | 61.74 | 687.08 | 31.39 | 812.54 | 20.72 | 382592 | 7.51 |
| binius-focused | 1/4 | 21 | binius64 | 6,798.51 | 56.53 | 144.18 | 1,510.67 | 64.07 | 1,769.63 | 33.84 | 412304 | 15.07 |
| binius-focused | 1/4 | 22 | binius64 | 13,991.76 | 112.84 | 255.93 | 2,479.67 | 107.90 | 2,965.78 | 61.37 | 446784 | 29.57 |
| binius-focused | 1/4 | 15 | binius64-ligerito | 82.48 | 0.83 | 2.64 | 42.61 | 9.87 | 55.90 | 3.81 | 232816 | 0.27 |
| binius-focused | 1/4 | 16 | binius64-ligerito | 171.56 | 1.72 | 5.63 | 63.09 | 15.32 | 86.23 | 4.39 | 241072 | 0.52 |
| binius-focused | 1/4 | 17 | binius64-ligerito | 365.57 | 3.41 | 10.02 | 107.66 | 18.24 | 138.53 | 5.13 | 250192 | 0.91 |
| binius-focused | 1/4 | 18 | binius64-ligerito | 752.01 | 6.81 | 19.21 | 192.21 | 30.18 | 248.85 | 5.99 | 266288 | 1.80 |
| binius-focused | 1/4 | 19 | binius64-ligerito | 1,561.80 | 13.61 | 37.95 | 414.74 | 48.16 | 512.99 | 10.06 | 277456 | 3.41 |
| binius-focused | 1/4 | 20 | binius64-ligerito | 3,679.65 | 27.48 | 77.06 | 853.72 | 76.47 | 1,036.73 | 16.48 | 287312 | 6.82 |
| binius-focused | 1/4 | 21 | binius64-ligerito | 6,328.92 | 54.18 | 146.19 | 1,640.84 | 199.64 | 2,045.72 | 27.21 | 304336 | 13.61 |
| binius-focused | 1/4 | 22 | binius64-ligerito | 14,013.02 | 106.69 | 289.46 | 2,925.60 | 241.59 | 3,603.24 | 55.32 | 316016 | 25.20 |
| binius-focused | 1/8 | 15 | binius64 | 80.05 | 0.94 | 4.04 | 44.12 | 5.15 | 54.49 | 2.22 | 242928 | 0.28 |
| binius-focused | 1/8 | 16 | binius64 | 169.50 | 1.84 | 7.82 | 64.07 | 6.17 | 79.21 | 2.46 | 259328 | 0.56 |
| binius-focused | 1/8 | 17 | binius64 | 348.36 | 3.62 | 14.67 | 105.20 | 8.99 | 138.84 | 3.04 | 283312 | 1.00 |
| binius-focused | 1/8 | 18 | binius64 | 748.18 | 7.27 | 27.31 | 193.27 | 13.55 | 250.12 | 4.25 | 306032 | 1.97 |
| binius-focused | 1/8 | 19 | binius64 | 1,502.78 | 14.15 | 54.65 | 344.28 | 27.18 | 440.41 | 12.07 | 330496 | 3.93 |
| binius-focused | 1/8 | 20 | binius64 | 3,391.25 | 28.01 | 117.03 | 745.04 | 39.48 | 938.67 | 20.25 | 355440 | 7.84 |
| binius-focused | 1/8 | 21 | binius64 | 7,097.53 | 57.29 | 290.05 | 1,496.93 | 91.68 | 1,985.43 | 37.06 | 383424 | 15.81 |
| binius-focused | 1/8 | 22 | binius64 | 13,954.88 | 140.76 | 558.62 | 2,646.76 | 149.17 | 3,467.56 | 67.18 | 408208 | 29.46 |
| binius-focused | 1/8 | 15 | binius64-ligerito | 90.22 | 0.85 | 4.32 | 43.54 | 8.99 | 57.92 | 3.67 | 186480 | 0.27 |
| binius-focused | 1/8 | 16 | binius64-ligerito | 175.94 | 1.70 | 9.37 | 63.19 | 11.14 | 85.06 | 4.01 | 194064 | 0.53 |
| binius-focused | 1/8 | 17 | binius64-ligerito | 363.43 | 3.37 | 16.05 | 104.34 | 16.68 | 140.08 | 4.98 | 200528 | 0.95 |
| binius-focused | 1/8 | 18 | binius64-ligerito | 754.88 | 6.73 | 33.47 | 174.10 | 28.41 | 245.71 | 6.13 | 216592 | 1.88 |
| binius-focused | 1/8 | 19 | binius64-ligerito | 1,574.85 | 13.63 | 66.56 | 386.96 | 46.75 | 518.41 | 9.04 | 223376 | 3.57 |
| binius-focused | 1/8 | 20 | binius64-ligerito | 3,113.03 | 27.78 | 141.25 | 890.20 | 201.18 | 1,283.97 | 14.71 | 233040 | 7.10 |
| binius-focused | 1/8 | 21 | binius64-ligerito | 6,769.64 | 56.50 | 301.95 | 1,752.03 | 233.59 | 2,347.09 | 26.61 | 247640 | 14.32 |
| binius-focused | 1/8 | 22 | binius64-ligerito | 14,295.21 | 107.91 | 617.23 | 3,559.13 | 403.58 | 4,663.86 | 78.25 | 256504 | 25.47 |
| all-provers | 1/2 | 15 | binius64 | 81.53 | 0.92 | 1.78 | 46.81 | 5.75 | 55.64 | 2.45 | 324080 | 0.28 |
| all-provers | 1/2 | 16 | binius64 | 172.46 | 1.84 | 3.14 | 66.41 | 6.54 | 77.92 | 2.88 | 355392 | 0.54 |
| all-provers | 1/2 | 17 | binius64 | 365.22 | 3.69 | 5.51 | 113.82 | 9.07 | 133.02 | 3.34 | 387888 | 0.92 |
| all-provers | 1/2 | 18 | binius64 | 762.96 | 7.23 | 9.56 | 175.12 | 11.50 | 204.09 | 4.46 | 431504 | 1.81 |
| all-provers | 1/2 | 19 | binius64 | 1,569.62 | 14.37 | 18.95 | 330.52 | 17.66 | 385.66 | 12.03 | 472256 | 3.67 |
| all-provers | 1/2 | 20 | binius64 | 3,224.71 | 27.96 | 31.20 | 727.30 | 30.14 | 816.41 | 22.27 | 518864 | 7.32 |
| all-provers | 1/2 | 21 | binius64 | 6,643.63 | 56.36 | 63.91 | 1,372.75 | 48.64 | 1,550.80 | 33.45 | 559200 | 14.73 |
| all-provers | 1/2 | 22 | binius64 | 13,715.28 | 112.92 | 139.46 | 2,489.12 | 107.65 | 2,882.76 | 62.93 | 606800 | 26.90 |
| all-provers | 1/2 | 15 | binius64-ligerito | 80.61 | 0.86 | 1.63 | 41.39 | 8.04 | 51.38 | 4.44 | 362080 | 0.27 |
| all-provers | 1/2 | 16 | binius64-ligerito | 178.37 | 1.68 | 3.11 | 59.34 | 11.33 | 75.69 | 4.97 | 373568 | 0.53 |
| all-provers | 1/2 | 17 | binius64-ligerito | 339.73 | 3.46 | 6.25 | 105.23 | 16.99 | 132.04 | 5.94 | 388352 | 0.90 |
| all-provers | 1/2 | 18 | binius64-ligerito | 758.38 | 6.78 | 11.03 | 204.78 | 28.56 | 257.82 | 7.11 | 408656 | 1.76 |
| all-provers | 1/2 | 19 | binius64-ligerito | 1,488.47 | 13.36 | 20.55 | 358.33 | 29.57 | 421.12 | 9.73 | 424816 | 3.35 |
| all-provers | 1/2 | 20 | binius64-ligerito | 3,178.48 | 27.24 | 48.71 | 789.45 | 66.70 | 928.13 | 15.93 | 438192 | 6.71 |
| all-provers | 1/2 | 21 | binius64-ligerito | 6,664.67 | 54.05 | 78.25 | 1,583.13 | 139.66 | 1,854.77 | 23.90 | 461872 | 13.38 |
| all-provers | 1/2 | 22 | binius64-ligerito | 14,186.33 | 107.72 | 172.67 | 3,072.76 | 344.98 | 3,716.22 | 61.67 | 477936 | 24.76 |
| all-provers | 1/2 | 15 | f2z | 11.77 | 0.07 | 1.17 | 2.34 | 22.26 | 26.17 | 3.98 | 118448 | 0.06 |
| all-provers | 1/2 | 16 | f2z | 22.74 | 0.15 | 1.89 | 3.43 | 30.39 | 36.05 | 3.58 | 138324 | 0.10 |
| all-provers | 1/2 | 17 | f2z | 44.95 | 0.25 | 2.91 | 5.10 | 50.92 | 59.63 | 4.56 | 150336 | 0.16 |
| all-provers | 1/2 | 18 | f2z | 89.14 | 0.52 | 5.00 | 7.82 | 90.90 | 107.69 | 4.99 | 168344 | 0.30 |
| all-provers | 1/2 | 19 | f2z | 185.21 | 1.02 | 9.64 | 13.15 | 152.98 | 180.33 | 5.83 | 188132 | 0.54 |
| all-provers | 1/2 | 20 | f2z | 366.77 | 2.09 | 18.04 | 24.67 | 275.41 | 331.02 | 5.86 | 211136 | 1.03 |
| all-provers | 1/2 | 21 | f2z | 768.55 | 4.12 | 33.64 | 57.03 | 580.54 | 673.94 | 8.37 | 226836 | 2.00 |
| all-provers | 1/2 | 22 | f2z | 1,466.62 | 8.20 | 75.14 | 87.87 | 1,206.65 | 1,379.54 | 8.17 | 262116 | 3.66 |
| all-provers | 1/2 | 15 | plonky3-fri | 17.16 | 3.57 | 310.71 | 34.92 | 54.58 | 409.67 | 28.94 | 621695 | 0.11 |
| all-provers | 1/2 | 16 | plonky3-fri | 17.18 | 7.88 | 607.60 | 67.29 | 104.33 | 793.11 | 31.60 | 687486 | 0.20 |
| all-provers | 1/2 | 17 | plonky3-fri | 17.39 | 15.77 | 1,504.11 | 155.90 | 227.14 | 1,883.17 | 35.09 | 763645 | 0.39 |
| all-provers | 1/2 | 18 | plonky3-fri | 17.59 | 31.30 | 2,653.30 | 258.36 | 436.46 | 3,417.66 | 39.09 | 854300 | 0.78 |
| all-provers | 1/2 | 19 | plonky3-fri | 17.76 | 60.43 | 5,501.23 | 593.59 | 848.93 | 7,109.71 | 43.13 | 937691 | 1.54 |
| all-provers | 1/2 | 20 | plonky3-fri | 17.60 | 90.77 | 10,793.04 | 1,056.99 | 1,641.06 | 13,693.41 | 44.92 | 1018138 | 4.15 |
| all-provers | 1/2 | 21 | plonky3-fri | 17.70 | 178.01 | 20,543.65 | 2,077.06 | 3,066.42 | 26,215.88 | 48.94 | 1139545 | 6.14 |
| all-provers | 1/2 | 22 | plonky3-fri | 18.90 | 392.60 | 43,174.55 | 4,340.02 | 6,072.62 | 54,433.87 | 53.59 | 1241080 | 12.27 |
| all-provers | 1/4 | 15 | binius64 | 76.99 | 0.90 | 3.04 | 43.72 | 5.07 | 52.72 | 2.27 | 253008 | 0.29 |
| all-provers | 1/4 | 16 | binius64 | 171.45 | 1.84 | 4.51 | 61.39 | 5.79 | 73.65 | 2.50 | 272416 | 0.54 |
| all-provers | 1/4 | 17 | binius64 | 367.55 | 3.63 | 8.66 | 105.55 | 7.97 | 125.03 | 3.16 | 297264 | 0.94 |
| all-provers | 1/4 | 18 | binius64 | 746.64 | 7.13 | 16.41 | 174.54 | 11.42 | 208.32 | 4.71 | 329248 | 1.85 |
| all-provers | 1/4 | 19 | binius64 | 1,680.78 | 14.22 | 34.54 | 361.96 | 19.10 | 426.59 | 11.47 | 354192 | 3.74 |
| all-provers | 1/4 | 20 | binius64 | 3,027.94 | 28.00 | 60.38 | 698.65 | 31.94 | 825.01 | 21.92 | 382592 | 7.51 |
| all-provers | 1/4 | 21 | binius64 | 6,477.23 | 57.10 | 133.06 | 1,425.48 | 53.63 | 1,670.57 | 33.09 | 412304 | 15.07 |
| all-provers | 1/4 | 22 | binius64 | 14,032.59 | 116.90 | 319.93 | 2,697.22 | 112.89 | 3,366.64 | 69.15 | 446784 | 25.85 |
| all-provers | 1/4 | 15 | binius64-ligerito | 80.47 | 0.84 | 3.06 | 42.73 | 10.13 | 56.86 | 3.83 | 232816 | 0.27 |
| all-provers | 1/4 | 16 | binius64-ligerito | 174.85 | 1.70 | 4.92 | 61.90 | 15.25 | 84.19 | 4.30 | 241072 | 0.52 |
| all-provers | 1/4 | 17 | binius64-ligerito | 355.57 | 3.41 | 9.28 | 98.34 | 16.72 | 126.85 | 5.08 | 250192 | 0.92 |
| all-provers | 1/4 | 18 | binius64-ligerito | 774.36 | 6.81 | 18.21 | 180.69 | 27.73 | 235.89 | 6.32 | 266288 | 1.81 |
| all-provers | 1/4 | 19 | binius64-ligerito | 1,435.42 | 13.42 | 34.95 | 384.18 | 42.20 | 480.82 | 9.07 | 277456 | 3.42 |
| all-provers | 1/4 | 20 | binius64-ligerito | 3,102.21 | 26.99 | 72.74 | 811.89 | 69.52 | 979.76 | 14.02 | 287312 | 6.82 |
| all-provers | 1/4 | 21 | binius64-ligerito | 6,375.64 | 53.18 | 140.48 | 1,597.99 | 189.58 | 1,977.35 | 26.38 | 304336 | 13.70 |
| all-provers | 1/4 | 22 | binius64-ligerito | 14,526.01 | 108.67 | 342.93 | 3,618.18 | 296.00 | 4,373.26 | 59.49 | 316016 | 23.16 |
| all-provers | 1/4 | 15 | f2z | 11.23 | 0.07 | 1.57 | 2.18 | 20.37 | 24.42 | 3.74 | 83840 | 0.06 |
| all-provers | 1/4 | 16 | f2z | 22.33 | 0.14 | 2.19 | 3.38 | 31.06 | 37.21 | 3.44 | 99908 | 0.10 |
| all-provers | 1/4 | 17 | f2z | 43.89 | 0.27 | 4.35 | 4.99 | 44.12 | 55.14 | 4.13 | 108400 | 0.17 |
| all-provers | 1/4 | 18 | f2z | 88.42 | 0.47 | 7.10 | 7.20 | 76.15 | 90.08 | 4.51 | 120488 | 0.30 |
| all-provers | 1/4 | 19 | f2z | 179.08 | 1.04 | 11.84 | 12.05 | 131.70 | 157.65 | 5.12 | 136012 | 0.54 |
| all-provers | 1/4 | 20 | f2z | 362.25 | 2.09 | 23.85 | 22.49 | 240.08 | 291.86 | 5.49 | 153696 | 1.06 |
| all-provers | 1/4 | 21 | f2z | 721.42 | 4.15 | 47.35 | 45.17 | 491.63 | 604.18 | 7.87 | 163828 | 1.95 |
| all-provers | 1/4 | 22 | f2z | 1,452.14 | 8.32 | 109.84 | 80.44 | 1,017.56 | 1,233.83 | 7.24 | 197124 | 3.90 |
| all-provers | 1/4 | 15 | plonky3-fri | 0.87 | 3.86 | 678.76 | 65.78 | 118.09 | 885.96 | 17.23 | 378943 | 0.20 |
| all-provers | 1/4 | 16 | plonky3-fri | 0.88 | 5.62 | 1,245.90 | 130.65 | 207.43 | 1,572.17 | 18.64 | 419289 | 0.39 |
| all-provers | 1/4 | 17 | plonky3-fri | 0.89 | 15.61 | 2,799.59 | 312.12 | 413.03 | 3,537.32 | 20.60 | 464851 | 0.77 |
| all-provers | 1/4 | 18 | plonky3-fri | 0.84 | 28.63 | 4,739.10 | 460.31 | 718.12 | 5,955.14 | 21.98 | 513165 | 1.80 |
| all-provers | 1/4 | 19 | plonky3-fri | 1.04 | 55.40 | 9,632.05 | 871.39 | 1,358.43 | 11,951.54 | 23.41 | 555975 | 3.06 |
| all-provers | 1/4 | 20 | plonky3-fri | 0.86 | 112.57 | 19,190.54 | 1,760.56 | 2,818.75 | 23,928.84 | 26.38 | 613249 | 6.10 |

## Evidence

[native-summary.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/native-summary.csv), [previous-vs-rerun.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/previous-vs-rerun.csv), [binius-changes.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/binius-changes.csv), [wide-summary.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/wide-summary.csv), [sha-summary.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/sha-summary.csv), [analysis.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/analysis.json), [source-sha256.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/source-sha256.json), [source.patch](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/source.patch)

Earlier campaign and statement/security audit: [previous report](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/REPORT.md).

Missing or incomplete cases:

```json
{
  "missing": {
    "native": [
      [
        "all-provers",
        2,
        "plonky3-fri",
        21
      ],
      [
        "all-provers",
        2,
        "plonky3-fri",
        22
      ],
      [
        "all-provers",
        3,
        "binius64",
        15
      ],
      [
        "all-provers",
        3,
        "binius64",
        16
      ],
      [
        "all-provers",
        3,
        "binius64",
        17
      ],
      [
        "all-provers",
        3,
        "binius64",
        18
      ],
      [
        "all-provers",
        3,
        "binius64",
        19
      ],
      [
        "all-provers",
        3,
        "binius64",
        20
      ],
      [
        "all-provers",
        3,
        "binius64",
        21
      ],
      [
        "all-provers",
        3,
        "binius64",
        22
      ],
      [
        "all-provers",
        3,
        "binius64-ligerito",
        15
      ],
      [
        "all-provers",
        3,
        "binius64-ligerito",
        16
      ],
      [
        "all-provers",
        3,
        "binius64-ligerito",
        17
      ],
      [
        "all-provers",
        3,
        "binius64-ligerito",
        18
      ],
      [
        "all-provers",
        3,
        "binius64-ligerito",
        19
      ],
      [
        "all-provers",
        3,
        "binius64-ligerito",
        20
      ],
      [
        "all-provers",
        3,
        "binius64-ligerito",
        21
      ],
      [
        "all-provers",
        3,
        "binius64-ligerito",
        22
      ],
      [
        "all-provers",
        3,
        "f2z",
        15
      ],
      [
        "all-provers",
        3,
        "f2z",
        16
      ],
      [
        "all-provers",
        3,
        "f2z",
        17
      ],
      [
        "all-provers",
        3,
        "f2z",
        18
      ],
      [
        "all-provers",
        3,
        "f2z",
        19
      ],
      [
        "all-provers",
        3,
        "f2z",
        20
      ],
      [
        "all-provers",
        3,
        "f2z",
        21
      ],
      [
        "all-provers",
        3,
        "f2z",
        22
      ],
      [
        "all-provers",
        3,
        "plonky3-fri",
        15
      ],
      [
        "all-provers",
        3,
        "plonky3-fri",
        16
      ],
      [
        "all-provers",
        3,
        "plonky3-fri",
        17
      ],
      [
        "all-provers",
        3,
        "plonky3-fri",
        18
      ],
      [
        "all-provers",
        3,
        "plonky3-fri",
        19
      ],
      [
        "all-provers",
        3,
        "plonky3-fri",
        20
      ],
      [
        "all-provers",
        3,
        "plonky3-fri",
        21
      ],
      [
        "all-provers",
        3,
        "plonky3-fri",
        22
      ],
      [
        "all-provers",
        null,
        "limber",
        15
      ],
      [
        "all-provers",
        null,
        "limber",
        16
      ],
      [
        "all-provers",
        null,
        "limber",
        17
      ],
      [
        "all-provers",
        null,
        "limber",
        18
      ],
      [
        "all-provers",
        null,
        "limber",
        19
      ],
      [
        "all-provers",
        null,
        "limber",
        20
      ],
      [
        "all-provers",
        null,
        "limber",
        21
      ],
      [
        "all-provers",
        null,
        "limber",
        22
      ]
    ],
    "wide": [],
    "sha": [
      [
        "binius64",
        1
      ],
      [
        "binius64",
        2
      ],
      [
        "binius64",
        3
      ],
      [
        "f2z-split",
        1
      ],
      [
        "f2z-split",
        2
      ],
      [
        "f2z-split",
        3
      ]
    ]
  },
  "incomplete": [
    {
      "source": "/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate2/plonky3-fri",
      "backend": "plonky3-fri",
      "exponent": 21,
      "records": 1
    }
  ]
}
```

Interactive phase report: [focused-intervals/intervals.html](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/focused-intervals/intervals.html). Numerical/structural validation is recorded alongside the report.
