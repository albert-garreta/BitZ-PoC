# Falcon-1024 algebraic proof thread scaling

32 signatures, 128-bit target, seed 42. One warm-up and five measured trials per configuration. Every proof was verified. Results are medians unless otherwise marked.

| Threads | Total prover (ms) | Speedup | Signatures/s | Verify (ms) | Payload (KiB) | Peak RSS (MiB) |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 102.977 | 1.00x | 310.7 | 9.770 | 152.74 | 26.4 |
| 2 | 56.784 | 1.81x | 563.5 | 7.492 | 152.74 | 26.2 |
| 4 | 34.194 | 3.01x | 935.8 | 6.384 | 152.74 | 28.0 |
| 8 | 22.647 | 4.55x | 1413.0 | 5.890 | 152.74 | 31.7 |
| 16 | 21.268 | 4.84x | 1504.6 | 5.848 | 152.74 | 39.7 |

## Timing boundaries and reproducibility

- Total prover includes centered witness reconstruction, native validation, slack/quotient preparation, commitment, and proof generation.
- Configuration setup, key/signature generation, public-target hashing, and input copies are outside total prover timing.
- Verification uses the configured global worker pool. It is not a single-thread verifier comparison.
- Peak RSS is process-wide, including input preparation and persistent caches; it is not an isolated prover allocation peak.
- Stored proof payload includes its commitment root and excludes public inputs and outer transport framing.
- Native release build: `-C target-cpu=native`, fat LTO, one codegen unit. Tracing disabled.
- AMD Ryzen 9 9950X3D. Workers pinned to physical CPUs 0..N-1; main and auxiliary workers pinned to CPU 0. No SMT or CPU quota.
- CPUs 0–7 have 96 MiB shared L3; CPUs 8–15 have 32 MiB shared L3. Sixteen threads crosses chiplets. Governor: powersave.
- Sequential execution order: 1, 16, 2, 8, 4. No simultaneous benchmark configurations.
- Arithmetic grinding searches the smallest valid nonce in both serial and parallel modes.
- Raw per-trial JSON, command metadata, source hashes and machine details are saved alongside this report.
- Shared input digest: `8e13abdaab7d13f141273f898c38b9eb8f9841f3efba64d329b8b4970ad18745`.

See `summary.csv` for phase medians, ranges and standard deviations.

## Phase medians

| Threads | Witness (ms) | Commit (ms) | Prove (ms) | Total range (ms) |
|---:|---:|---:|---:|---:|
| 1 | 2.925 | 5.295 | 94.755 | 102.848–103.680 |
| 2 | 1.480 | 3.465 | 51.785 | 56.672–57.626 |
| 4 | 0.788 | 2.727 | 30.314 | 33.777–34.597 |
| 8 | 0.407 | 2.387 | 19.736 | 22.385–23.089 |
| 16 | 0.418 | 2.375 | 18.504 | 20.781–21.596 |

Phase medians are calculated independently, so their sum need not equal the median total.

## Validation

46 tests passed: 19 algebraic, 14 shared bridge, 12 shared sumcheck, and one existing full-Falcon round-trip/tampering regression. The large-batch qualification test remains ignored; this sweep independently exercises batch 32 at the 128-bit target.

Validation found and fixed a small-batch PCS configuration error: batches below eight slots now receive authenticated zero padding to meet the existing minimum PCS domain. The batch-32 geometry used here is unchanged.

All 30 benchmark proofs (five warm-ups and 25 measured proofs) verified. Every run used the same input digest, and all source hashes were checked after the sweep.

At 16 threads, throughput is 4.84 times the one-thread baseline; moving from eight to sixteen threads adds about 6.5% throughput for this batch. The sixteen-thread case crosses the CPU’s two chiplets, so these measurements do not isolate the cause of that flattening.
