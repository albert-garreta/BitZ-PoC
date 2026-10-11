# Eight-seed v7 qualification before the next performance edits

This expanded run does **not pass** the strict no-regression gate. Seventeen of 24 shapes improve both total prover and proof-only medians; seven shapes have positive observed changes, with the largest proof-only increase 1.21%. These small changes require additional independent-input coverage and component analysis. They are retained here without labeling them established regressions or accepting them as no regression.

Both degrees, security 100/128, batches 1/32/1024 and one/eight threads were tested with the same eight seeds 42–49, fixed before running. Every seed had one warmup and three measured proofs. All 1152 measured proofs and 384 warmups verified; all baseline/candidate input digests matched. The 128 proofs shared with the first v7 run exactly match its source roots, full proof Debug digests and payload sizes (`reference-proof-identity.json`).

Positive Δ means slower. Times are milliseconds, with medians over 24 measured proofs per variant/shape. A positive delta in either timing metric fails the zero-tolerance observed gate.

| N | Security | Batch | Threads | Total before → after | Total Δ | Proof-only before → after | Proof Δ |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | 1 | 13.8188 → 13.7714 | -0.343% | 13.6545 → 13.6071 | -0.347% |
| 512 | 100 | 1 | 8 | 4.3451 → 4.3959 | +1.167% | 4.1871 → 4.2375 | +1.206% |
| 512 | 100 | 32 | 1 | 17.5217 → 17.6477 | +0.719% | 16.3651 → 16.4703 | +0.643% |
| 512 | 100 | 32 | 8 | 5.5866 → 5.5980 | +0.206% | 5.2170 → 5.2225 | +0.104% |
| 512 | 100 | 1024 | 1 | 270.0408 → 265.3860 | -1.724% | 234.6479 → 228.2083 | -2.744% |
| 512 | 100 | 1024 | 8 | 54.5340 → 54.3414 | -0.353% | 48.9680 → 48.7008 | -0.546% |
| 512 | 128 | 1 | 1 | 17.2602 → 17.0772 | -1.060% | 17.0957 → 16.9101 | -1.086% |
| 512 | 128 | 1 | 8 | 6.8339 → 6.8417 | +0.113% | 6.6658 → 6.6794 | +0.204% |
| 512 | 128 | 32 | 1 | 21.1028 → 21.2483 | +0.690% | 19.9597 → 20.0689 | +0.547% |
| 512 | 128 | 32 | 8 | 8.0713 → 7.9945 | -0.952% | 7.7079 → 7.6093 | -1.280% |
| 512 | 128 | 1024 | 1 | 281.6405 → 273.5743 | -2.864% | 246.6784 → 236.3486 | -4.188% |
| 512 | 128 | 1024 | 8 | 59.4465 → 59.7001 | +0.427% | 53.7949 → 53.9415 | +0.273% |
| 1024 | 100 | 1 | 1 | 13.8701 → 13.8860 | +0.115% | 13.6408 → 13.6566 | +0.116% |
| 1024 | 100 | 1 | 8 | 4.5447 → 4.4487 | -2.112% | 4.3234 → 4.2198 | -2.396% |
| 1024 | 100 | 32 | 1 | 27.0416 → 26.2603 | -2.889% | 23.7244 → 22.9668 | -3.193% |
| 1024 | 100 | 32 | 8 | 8.1678 → 7.9952 | -2.113% | 7.2515 → 7.0669 | -2.545% |
| 1024 | 100 | 1024 | 1 | 504.7420 → 493.5507 | -2.217% | 402.1490 → 389.2606 | -3.205% |
| 1024 | 100 | 1024 | 8 | 107.8505 → 106.7887 | -0.984% | 92.7359 → 91.9503 | -0.847% |
| 1024 | 128 | 1 | 1 | 17.4103 → 17.2532 | -0.903% | 17.1848 → 17.0249 | -0.931% |
| 1024 | 128 | 1 | 8 | 7.0552 → 7.0174 | -0.536% | 6.8192 → 6.7661 | -0.779% |
| 1024 | 128 | 32 | 1 | 31.1273 → 30.1724 | -3.068% | 27.7992 → 26.8778 | -3.315% |
| 1024 | 128 | 32 | 8 | 10.8469 → 10.6380 | -1.926% | 9.9291 → 9.6710 | -2.599% |
| 1024 | 128 | 1024 | 1 | 523.7324 → 505.7701 | -3.430% | 420.7690 → 401.6902 | -4.534% |
| 1024 | 128 | 1024 | 8 | 114.9500 → 113.4452 | -1.309% | 99.9114 → 98.4479 | -1.465% |

Prime search and grinding are deterministic for an input/transcript but change across protocol domains. Repeated trials do not replace independent input seeds. `seed-summary.json` separates these inputs; the gate does not claim statistical confidence. Native release builds, pinned worker placement, sequential variant runs and alternating variant order follow the earlier matrix. No builds or test suites ran concurrently. Brief progress aggregation used CPU 15, outside benchmark workers 0–7.

The next qualification is fixed in advance: all 16 small shapes (batches 1/32) use seeds 42–73; all 8 large shapes (batch 1024) use seeds 42–49. Earlier evidence is not overwritten.

Artifacts: `summary.json`, `seed-summary.json`, `regression-gate.json`, `samples.json`, per-run command/log files, `metadata.json` and `reference-proof-identity.json`. Final-v7 small-case diagnostic profiles are in the sibling `v7-final-small-profiles/` directory.
