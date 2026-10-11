# Final-v7 small Falcon-512 diagnostic profiles

This compares the actual eight-seed-run binary against the preserved original v5 baseline, at security 100, batches 1/32, one/eight threads and seeds 42/43. Each run has one warmup and three measured verified proofs. All 48 measured proofs and 16 warmups verified, and all 32 candidate profile proofs exactly match the untraced v7-final proofs. Traces are diagnostic and are not substituted for untraced latency measurements.

Table entries are candidate-minus-baseline milliseconds. `Other` includes protocol setup, uninstrumented prime sampling, compact claim preparation and joint sumcheck; `Joint setup` is nested inside it. Do not add them.

| Batch | Threads | Seed | Packing Δ | Ring Δ | Arithmetic Δ | Bridge Δ | PCS Δ | Other Δ | Joint setup Δ |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 42 | +0.0028 | -0.0377 | -0.1033 | +0.0450 | +0.0042 | +0.1719 | -0.0134 |
| 1 | 1 | 43 | +0.0025 | -0.0382 | -0.0389 | +0.0276 | +0.0232 | +0.0042 | -0.0137 |
| 1 | 8 | 42 | -0.0140 | -0.0461 | -0.0266 | -0.0038 | -0.0106 | +0.1727 | -0.0125 |
| 1 | 8 | 43 | -0.0075 | -0.0394 | -0.0198 | -0.0051 | +0.0184 | +0.0044 | -0.0092 |
| 32 | 1 | 42 | +0.0113 | -0.0351 | -0.0404 | +0.0556 | -0.0151 | -0.0006 | -0.0146 |
| 32 | 1 | 43 | +0.0102 | -0.0190 | -0.0138 | -0.0041 | +0.0149 | -0.0160 | -0.0151 |
| 32 | 8 | 42 | +0.0120 | -0.0547 | +0.0090 | +0.0143 | +0.0116 | -0.0092 | -0.0111 |
| 32 | 8 | 43 | +0.0145 | -0.0551 | +0.0029 | -0.0211 | +0.0220 | -0.0286 | -0.0152 |

Joint packed setup is now faster by approximately 0.01–0.015ms, eliminating the earlier two-tensor overhead. Ring time also improves. Batch 32 packing adds approximately 0.01–0.014ms.

For batch 1, seed 42 adds approximately 0.172ms to Other at both one and eight threads; seed 43 adds only 0.004ms at either thread count. This supports an input-dependent serial-setup explanation, but the prime sampler is not instrumented in these binaries, so its exact contribution remains unknown. The current traces do not establish that prime search caused the residual difference. Batch 32 Other is flat or improved for both measured seeds.

`stage-summary.json` contains all per-run stage medians with warmups excluded. Future binaries add prime/carry spans; those spans are absent from this preserved run.
