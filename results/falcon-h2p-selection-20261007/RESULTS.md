# Full Falcon: public HashToPoint selection

All 20 cases passed the recorded prover/verifier timing gate. Peak RSS is reported separately and does not determine acceptance.

1,024 distinct original-Falcon signatures per proof; dimensions 512 and 1024; 100- and 128-bit security targets; 1, 2, 4, 8 and 16 threads. Each case uses seed 42, one warm-up and five verified measured trials on `will` (AMD Ryzen 9 9950X3D). The tables show medians. Inputs use `pornin/rust-fn-dsa` 0.3.0. Compared with the [archived baseline](../falcon-power-basis-20261007/results-candidate/); no new baseline runs are included.

Native release build: `-C target-cpu=native`, fat LTO, one codegen unit. CPU affinity, input digests, compiled features and per-trial metadata are in the [raw logs](raw/); revision, file hashes and binary identification are in [metadata.json](metadata.json). Protocol: `bitz/falcon/shared-prime/non-zk/v4`; source layout: `aligned16-h2p-selection-v4`.

Prover time includes checked witness preparation, packing, commitment and proving. Input generation and reusable parameter preparation are excluded. Each timing gate requires a candidate/baseline median ratio at most 1 and an independently resampled, one-sided 95% bootstrap upper mean-ratio bound at most 1 (20,000 resamples, RNG seed 0). This compares five fixed-input samples from each archive; it does not measure variation across input seeds. Peak RSS is the process cumulative high-water mark, including setup, warm-up and earlier trials.

The measured change combines public-mask linear routing, the factored verifier binder and optimized K=4 packed-prefix processing. These end-to-end results do not isolate Horner evaluation, the rejected tree, or mask routing as individual kernels.

| N | Security | Threads | Prover ms: before → after | Verify ms: before → after | Peak RSS MiB: before → after | Timing gate |
|---:|---:|---:|---:|---:|---:|:---:|
| 512 | 100 | 1 | 2427.84 → 2221.87 | 233.46 → 69.07 | 2257.58 → 2304.37 | PASS |
| 512 | 100 | 2 | 1290.41 → 1174.31 | 134.79 → 48.97 | 2303.91 → 2302.92 | PASS |
| 512 | 100 | 4 | 722.38 → 662.12 | 78.17 → 39.63 | 2350.07 → 2280.66 | PASS |
| 512 | 100 | 8 | 446.94 → 411.12 | 53.99 → 34.75 | 2362.52 → 2305.44 | PASS |
| 512 | 100 | 16 | 357.06 → 321.96 | 41.04 → 32.31 | 2369.03 → 2321.98 | PASS |
| 512 | 128 | 1 | 3681.99 → 3246.24 | 235.89 → 71.25 | 2250.39 → 2309.70 | PASS |
| 512 | 128 | 2 | 1923.34 → 1703.27 | 130.49 → 51.21 | 2309.18 → 2312.32 | PASS |
| 512 | 128 | 4 | 1056.39 → 938.63 | 80.63 → 41.80 | 2358.45 → 2326.14 | PASS |
| 512 | 128 | 8 | 640.88 → 569.64 | 56.24 → 37.11 | 2355.60 → 2333.47 | PASS |
| 512 | 128 | 16 | 474.47 → 419.72 | 43.44 → 34.64 | 2366.49 → 2336.62 | PASS |
| 1024 | 100 | 1 | 4558.49 → 4106.95 | 422.30 → 126.63 | 3260.77 → 3199.75 | PASS |
| 1024 | 100 | 2 | 2459.53 → 2187.71 | 253.71 → 87.72 | 3222.89 → 3128.25 | PASS |
| 1024 | 100 | 4 | 1397.22 → 1240.69 | 155.70 → 69.00 | 3258.22 → 3205.08 | PASS |
| 1024 | 100 | 8 | 884.54 → 788.48 | 94.42 → 59.67 | 3272.68 → 3211.59 | PASS |
| 1024 | 100 | 16 | 688.54 → 605.08 | 70.65 → 54.71 | 3304.10 → 3242.61 | PASS |
| 1024 | 128 | 1 | 7416.30 → 5876.04 | 450.45 → 129.87 | 3257.12 → 3222.34 | PASS |
| 1024 | 128 | 2 | 3892.33 → 3103.67 | 255.93 → 91.07 | 3224.46 → 3224.09 | PASS |
| 1024 | 128 | 4 | 2168.43 → 1748.47 | 157.70 → 72.86 | 3253.02 → 3220.00 | PASS |
| 1024 | 128 | 8 | 1316.95 → 1086.81 | 97.89 → 63.29 | 3315.11 → 3175.32 | PASS |
| 1024 | 128 | 16 | 957.54 → 799.55 | 74.47 → 58.47 | 3299.79 → 3267.80 | PASS |

The [comparison JSON](comparison.json) retains individual ratios and confidence bounds.

## Witness and proof size

| N | Live arithmetic bits/signature: before → after | Padded arithmetic bits/signature: before → after | Total packed source KiB/signature: before → after |
|---:|---:|---:|---:|
| 512 | 59,817 → 52,637 | 65,536 → 65,536 | 96 → 96 |
| 1024 | 114,914 → 100,482 | 131,072 → 131,072 | 176 → 176 |

Total packed source includes arithmetic and every SHAKE slab: 96 MiB / 176 MiB for the 1,024-signature batches. These are logical source sizes, not peak process memory. The public masks are proof bytes and are not additional committed source columns.

| N | Security | Full payload KiB: before → after | Public-mask bytes/signature | Public masks KiB/batch, included in payload |
|---:|---:|---:|---:|---:|
| 512 | 100 | 482.92 → 566.72 | 90 | 90 |
| 512 | 128 | 600.25 → 681.46 | 90 | 90 |
| 1024 | 100 | 640.20 → 796.12 | 164 | 164 |
| 1024 | 128 | 794.62 → 950.35 | 164 | 164 |

Payloads are the benchmark's canonical stored payload, excluding Falcon outer framing and the public statement; masks are included. Ranges, if any, span thread configurations. Public masks add exactly 92,160 / 167,936 bytes for 1,024 Falcon-512 / Falcon-1024 signatures; removed HashToPoint proof messages and PCS multiproof overlap also affect the final payload. Measured full payload increases by 81.21–155.91 KiB per batch.

## Proved constraints

The protocol remains non-ZK and retains one initial joint source commitment. For each candidate j it proves W_j = 12289 Q_j + R_j, with W_j encoded in 16 bits, Q_j in three bits, and bounded14 decoding enforcing 0 ≤ R_j ≤ 12288. It proves the quadratic rejection identity e_j = Q_{j,2} Q_{j,0}. Canonical public masks have exactly N set bits, zero unused tail bits, and are absorbed before ring and arithmetic challenges. Through the last selected position, linear rows enforce e_j = 1 − a_j; increasing set-bit positions i_k define linear bindings C_k = R_{i_k}. These constraints force the first N accepted residues in order. Scalar residual bounds are below the arithmetic prime.

Full SHAKE-256 verification, SHAKE-to-word links, Falcon ring membership and norm checks remain proved and authenticated against the shared sources. HashToPoint has no grand product, GKR compaction, polynomial-tree witness or committed Horner states. The integer-to-binary BitZ bridge still uses its existing GKR proof, and the recursive PCS still has its existing internal commitments. Algebraic Falcon's relation, encoding and proof protocol are unchanged.

Validation: **760 tests passed**; see the [test log](tests.log). Benchmark validation also checks every warm-up and measured proof, canonical masks, matched input digests and public parameters, and the composed security target.

After the timing campaign, cleanup removed unused native prefix-counter storage, compaction parameter accessors, and obsolete test-only encoded-word helpers. The [cleanup validation](cleanup-validation.json) records another 760 passing tests and four verified 1,024-signature proofs covering both dimensions and security targets. Their proof Debug digests, payload sizes, input digests, and source roots match all six corresponding archived trials. The [cleanup test log](cleanup-tests.log) and source hashes are retained separately; the timing tables above remain the original measurements.

## Retained experiments

The [balanced polynomial-tree candidate](history/tree-comparison.json) was rejected (20 prover and 2 verifier timing failures). It added committed U/V coefficient tables. The [initial public-selection candidate](history/selection-v1-comparison.json) passed 20/20 timing cases; 3 peak-RSS measurements increased, by at most 2.64%. Memory differences are informational under the final timing-only gate. The selected implementation is this initial public-selection candidate. A later coefficient-allocation slab experiment reduced memory but slowed proving in all 20 cases, by 0.4–9.1%; it was reverted to prioritize speed. Its [comparison](history/slab-comparison.json) is retained as an allocation experiment, with matching proof Debug digests for all 120 runs. The tables above describe the retained faster implementation.

## Reproduce the report

From this archive directory (the benchmark logs already exist):

```sh
python3 ../../scripts/compare_falcon_h2p.py raw --baseline ../falcon-power-basis-20261007/results-candidate/ > comparison.json
python3 make_report.py --candidate-dir raw --comparison comparison.json --out RESULTS.md --metadata metadata.json --tests-passed 760
```

The generator validates the complete 20-case matrix and checks raw timing, payload, RSS and source-size values against the comparison before writing the report.
