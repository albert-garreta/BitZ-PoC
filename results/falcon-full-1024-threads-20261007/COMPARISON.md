# Algebraic and full Falcon-1024 proofs, batch 1,024

Same 1,024 original-Falcon signatures, 128-bit target, seed 42, native release build, and physical-core affinity policy. Each cell is a median of five trials after one warm-up. Every proof verified.

| Threads | Algebraic total prover (ms) | Full total prover (ms) | Algebraic verify (ms) | Full verify (ms) | Full/algebraic prover |
|---:|---:|---:|---:|---:|---:|
| 1 | 2504.356 | 6412.733 | 211.616 | 441.908 | 2.56x |
| 2 | 1368.290 | 3393.609 | 139.138 | 257.575 | 2.48x |
| 4 | 810.817 | 1916.715 | 102.376 | 159.074 | 2.36x |
| 8 | 530.152 | 1185.874 | 82.905 | 98.087 | 2.24x |
| 16 | 409.181 | 891.944 | 75.114 | 73.968 | 2.18x |

- Algebraic proves the ring relation and exact per-signature norm for public h,t and committed s1,s2. SHAKE and HashToPoint are external.
- Full proves complete verification, including SHAKE-256 and HashToPoint; public inputs include public key, message, signature nonce and s2.
- Total prover includes witness preparation, commitment and proof. Input generation/setup and upstream native verification are excluded.
- These are different proof compositions and public interfaces; their timing difference is not an isolated SHAKE/HashToPoint component measurement.
- Verification uses configured global workers; full also uses a serial Flock Keccak verifier on CPU 0.
- Including the 32-byte source commitment root, payloads are 472,890 bytes (461.807 KiB) algebraic and 814,586 bytes (795.494 KiB) full. Public polynomials/signature data and transport framing are excluded.
- Raw full payload is 814,554 bytes because its source root lives in the public statement. Raw algebraic payload already includes the root.
- Input digest: `de99e653487b5daa5bc850062b51a776a9f1c15e330694d23400913b231eba39`.
- Source files match the earlier 32-signature experiment. Algebraic binary is byte-identical; full harness was built from the current source.
- 60 proofs verified across both sweeps. Full proof debug digests were identical across all 30 full trials.

See REPORT.md, summary.csv and raw threads-N.jsonl for full-run details; ../falcon-algebraic-1024-threads-20261007/ contains the algebraic sweep.
