# Full Falcon-1024 verification proof thread scaling

1,024 signatures, 128-bit target, seed 42. One warm-up and five measured trials per configuration. Every proof verified. Includes SHAKE-256 and HashToPoint. Medians unless otherwise marked.

| Threads | Total prover (ms) | Speedup | Signatures/s | Verify (ms) | Payload (KiB) | Peak RSS (MiB) |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 6412.733 | 1.00x | 159.7 | 441.908 | 795.46 | 3222.9 |
| 2 | 3393.609 | 1.89x | 301.7 | 257.575 | 795.46 | 3224.8 |
| 4 | 1916.715 | 3.35x | 534.2 | 159.074 | 795.46 | 3226.6 |
| 8 | 1185.874 | 5.41x | 863.5 | 98.087 | 795.46 | 3276.3 |
| 16 | 891.944 | 7.19x | 1148.1 | 73.968 | 795.46 | 3308.2 |

## Timing boundaries and reproducibility

- Total prover includes witness generation (SHAKE, HashToPoint, ring and norm data), packing, commitment, statement clone, and proof generation.
- Configuration, key/signature generation, public parsing, upstream native verification, proof debug digest, and payload accounting are outside prover timing.
- Verification combines the configured global pool with a serial Flock Keccak verifier pinned to CPU 0.
- Peak RSS is process-wide, including input preparation and persistent caches.
- Proof size is canonical stored proof payload; excludes the public statement, its 32-byte source root, and outer transport framing.
- Native release build: -C target-cpu=native, fat LTO, one codegen unit. Stage timings disabled.
- AMD Ryzen 9 9950X3D. Workers on physical CPUs 0..N-1; main and auxiliary workers on CPU 0. No SMT.
- CPUs 0–7 share 96 MiB L3; CPUs 8–15 share 32 MiB L3. The 16-thread run crosses chiplets. Governor: powersave.
- Sequential execution order: 1, 16, 2, 8, 4. No competing benchmark or compilation was started by this agent.
- Same original-Falcon corpus as the algebraic 1024 sweep.
- Input digest: `de99e653487b5daa5bc850062b51a776a9f1c15e330694d23400913b231eba39`.

Raw records, metadata, source hashes, and CSV phase statistics are saved alongside this report.

All 30 full proofs verified and had identical source roots, proof debug digests, and payload sizes across thread counts. Source hashes were checked after the sweep. Adding the external 32-byte source commitment root gives 814,586 bytes (795.494 KiB).
