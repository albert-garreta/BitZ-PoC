# Falcon-1024 algebraic proof thread scaling

1,024 signatures, 128-bit target, seed 42. One warm-up and five measured trials per configuration. Every proof was verified. Results are medians unless otherwise marked.

| Threads | Total prover (ms) | Speedup | Signatures/s | Verify (ms) | Payload (KiB) | Peak RSS (MiB) |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2504.356 | 1.00x | 408.9 | 211.616 | 461.81 | 379.3 |
| 2 | 1368.290 | 1.83x | 748.4 | 139.138 | 461.81 | 379.1 |
| 4 | 810.817 | 3.09x | 1262.9 | 102.376 | 461.81 | 420.4 |
| 8 | 530.152 | 4.72x | 1931.5 | 82.905 | 461.81 | 457.4 |
| 16 | 409.181 | 6.12x | 2502.6 | 75.114 | 461.81 | 494.2 |

## Timing boundaries and reproducibility

- Total prover includes centered witness reconstruction, native validation, slack/quotient preparation, commitment, and proof generation.
- Configuration setup, key/signature generation, public-target hashing, and input copies are outside total prover timing.
- Verification uses the configured global worker pool. It is not a single-thread verifier comparison.
- Peak RSS is process-wide, including input preparation and persistent caches; it is not an isolated prover allocation peak.
- Stored proof payload includes its commitment root and excludes public inputs and outer transport framing.
- Native release build: `-C target-cpu=native`, fat LTO, one codegen unit. Tracing disabled.
- AMD Ryzen 9 9950X3D. Workers pinned to physical CPUs 0..N-1; main and auxiliary workers pinned to CPU 0. No SMT.
- CPUs 0–7 have 96 MiB shared L3; CPUs 8–15 have 32 MiB shared L3. Sixteen threads crosses chiplets. Governor: powersave.
- Sequential execution order: 1, 16, 2, 8, 4. No simultaneous benchmark configurations.
- Arithmetic grinding searches the smallest valid nonce in both serial and parallel modes.
- Raw per-trial JSON, command metadata, source hashes and machine details are saved alongside this report.
- Shared input digest: `de99e653487b5daa5bc850062b51a776a9f1c15e330694d23400913b231eba39`.

See `summary.csv` for phase medians, ranges and standard deviations.
