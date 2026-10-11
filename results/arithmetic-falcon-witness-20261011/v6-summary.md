# Arithmetic Falcon witness optimization measurements

The final implementation reduces maximum live witness bits by about 20%, physical committed source bits by 18.75% above the PCS floor, and measured proof payloads by 2.3–12.0%. Proving-time improvements are modest and depend on the configuration: batch-1024 single-thread medians improve 1.2–3.5%, while eight-thread batch-1024 results are approximately unchanged to 2.2% slower. The measurements do not establish a broad proving-speed improvement.

## Witness and allocation sizes

| Quantity | Falcon-512 | Falcon-1024 |
|---|---:|---:|
| Maximum live bits per signature, before → after | 15,386 → 12,338 (−19.81%) | 30,747 → 24,651 (−19.83%) |
| Physical source at batch 32 | 64 → 64 KiB | 128 → 104 KiB |
| Physical source at batch 1024 | 2 → 1.625 MiB | 4 → 3.25 MiB |
| Removed batched ring certificate | 11,242 bytes | 22,506 bytes |

Live counts include maximum exception capacity and slack. Actual exception indices add `2 + 2*count` proof bytes per signature; the tested original-Falcon inputs used zero exceptions. Physical source figures include padded batch capacity and opening source padding, but exclude Reed–Solomon expansion. The aligned virtual arithmetic view remains unchanged. Physical savings are 18.75% when the three sources can be split; the minimum PCS allocation prevents savings for Falcon-512 at batch 32.

## Final candidate latency and payload

Each cell reports baseline → final candidate. Times are milliseconds; payloads are KiB (1024 bytes). Δ is the candidate prover time relative to baseline, so positive means slower. Each value is the median of six measured proofs.

| Degree | Security | Batch | Threads | Prover ms | Δ | Verify ms | Payload KiB |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 32 | 1 | 17.46 → 17.64 | +1.1% | 3.68 → 3.85 | 92.22 → 81.20 |
| 512 | 100 | 32 | 8 | 5.62 → 5.78 | +2.9% | 3.18 → 3.28 | 92.22 → 81.20 |
| 512 | 100 | 1024 | 1 | 269.73 → 266.45 | -1.2% | 43.82 → 45.25 | 220.73 → 211.36 |
| 512 | 100 | 1024 | 8 | 54.99 → 56.18 | +2.2% | 19.53 → 20.03 | 220.73 → 211.36 |
| 512 | 128 | 32 | 1 | 21.14 → 21.05 | -0.5% | 4.30 → 4.36 | 109.07 → 98.36 |
| 512 | 128 | 32 | 8 | 7.99 → 8.25 | +3.3% | 3.76 → 3.81 | 109.07 → 98.36 |
| 512 | 128 | 1024 | 1 | 282.04 → 273.43 | -3.1% | 44.46 → 46.02 | 257.33 → 251.45 |
| 512 | 128 | 1024 | 8 | 60.24 → 61.20 | +1.6% | 20.36 → 21.11 | 257.33 → 251.45 |
| 1024 | 100 | 32 | 1 | 26.98 → 26.32 | -2.5% | 5.24 → 5.27 | 115.13 → 102.09 |
| 1024 | 100 | 32 | 8 | 8.12 → 8.10 | -0.2% | 4.21 → 4.16 | 115.13 → 102.09 |
| 1024 | 100 | 1024 | 1 | 503.95 → 493.77 | -2.0% | 86.87 → 89.58 | 307.08 → 270.64 |
| 1024 | 100 | 1024 | 8 | 108.21 → 108.94 | +0.7% | 37.35 → 36.71 | 307.08 → 270.64 |
| 1024 | 128 | 32 | 1 | 31.38 → 30.20 | -3.8% | 5.80 → 5.81 | 134.27 → 124.34 |
| 1024 | 128 | 32 | 8 | 11.07 → 10.58 | -4.5% | 4.75 → 4.68 | 134.27 → 124.34 |
| 1024 | 128 | 1024 | 1 | 523.15 → 504.98 | -3.5% | 87.07 → 88.75 | 353.32 → 312.91 |
| 1024 | 128 | 1024 | 8 | 114.90 → 115.00 | +0.1% | 37.57 → 37.64 | 353.32 → 312.91 |

The largest measured prover improvement is 4.5% for Falcon-1024/security-128/batch-32/eight threads. The largest regression is 3.3% for Falcon-512/security-128/batch-32/eight threads. Single-thread verification is generally slightly slower, as the table shows. Two input seeds limit confidence in small timing differences.

## Method and limits

- Native release builds with `RUSTFLAGS="-C target-cpu=native"`, `--features falcon-hybrid`, `--locked --offline`, identical compiler/profile settings. Baseline source hashes and binary hashes are recorded.
- AMD Ryzen 9 9950X3D; one thread pinned to CPU 6, or eight workers pinned to CPUs 6,7,4,5,2,3,0,1. Both Rayon pools use the same worker count. Verification uses the configured pool.
- Six measured proofs per variant/shape: seeds 42 and 43, each with one warmup and three measured trials. Processes run sequentially; baseline/candidate order reverses for the second seed. No concurrent builds or test runs.
- In the final matrix, all 192 measured proofs and 64 warmups verified. Every matched pair had the same input digest. All 128 final-candidate rows also exactly matched the initial candidate’s input digest, source root, full proof Debug digest and payload size, confirming that the final performance changes preserved proof contents. Input generation, decoding and external hashing are outside prover timing; witness derivation, packing, commitment and proof generation are included.
- Repetitions of one input reuse its deterministic transcript and grinding work. Two seeds provide limited coverage of that variation; small percentage differences should not be treated as statistically established speedups.
- Payload accounting includes stored proof scalars and PCS payload, excludes public inputs/transport framing, and is not Rust heap usage or a specified wire codec.
- Diagnostic traces are separate from these latency runs. Nested spans overlap; their times must not be summed indiscriminately.

## Diagnostic findings

Separate traces of the initial candidate identified added work in ring coordinate generation and compact packing. At security 100, batch 1024 and one thread, coordinate generation grew from 5.52 to 10.39 ms for Falcon-512 and from 11.60 to 20.97 ms for Falcon-1024. PCS proving improved from 11.85 to 7.14 ms and 23.03 to 13.51 ms, but the extra coordinate and packing work erased those gains.

The final implementation uses a fused SIMD recurrence for ring weights, packs compact planes word by word, and factors repeated slack gathers. The full final matrix above measures these changes together. Exact proof-identity checks distinguish these performance changes from protocol changes. Initial traces remain available as diagnostic evidence; they are not final-stage timings.

The norm proof, arithmetic binder and BitZ bridge still operate on the aligned virtual witness. Their arithmetic size does not fall with the compact commitment size. The minimum PCS allocation and compact-decoder work also limit speed gains, especially for small Falcon-512 batches.

## Artifacts

- `final/summary.json`: final per-shape medians and percentage reductions.
- `final/samples.json`, `final/*.jsonl`, `final/*.command.json`: final samples, warmups, exact commands and affinity readback.
- `final/metadata.json`: final binary hashes, build settings, machine data and reference-identity check status.
- `baseline-source-sha256.json`, `candidate-final-source-sha256.json`, `baseline-build.log`: source and build provenance.
- Root-level `summary.json`, `samples.json`, run logs and `metadata.json`: initial-candidate matrix, retained for comparison.
- `profiles/` and `profiles100/`: initial-candidate security-128 and security-100 diagnostic traces, with `stage-summary.json` excluding warmups.
- `benchmark.py`, `analyze_profiles.py`: measurement and analysis scripts.
- `VALIDATION.md`: implementation test results recorded separately by the main agent.
