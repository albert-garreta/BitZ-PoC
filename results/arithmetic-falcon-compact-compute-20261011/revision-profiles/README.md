# Revised production component diagnosis (R4)

All **176 diagnostic proofs verified**; all **88 candidate/control proof comparisons** and **176 matches to the untraced v7 qualification proof** passed. The four batch-32/one-worker degree/security shapes use seed 42, two warmups and 20 measured traces per binary. Binary order reverses between successive shapes. No other builds, tests, profiles or analysis ran concurrently.

The main remaining cost is inside the existing arithmetic binding, especially `inner_packed:fold_tail`. The root residual is largely unchanged or smaller. Tracing amplifies/distorts some differences, and one input cannot characterize independent-seed performance; this diagnosis does not replace the [failed untraced check](../revision-batch32/README.md).

All deltas below are candidate minus v7 in microseconds, computed as differences between per-stage medians. Stages can overlap/nest, and differences between separate medians need not add to the difference between root medians. Root residual is computed per proof as the root duration minus ring, arithmetic, BitZ and PCS spans before taking its median.

| N | Security | Proof root Δ µs | Arithmetic Δ µs | Binding Δ µs | Fold tail Δ µs | Ring Δ µs | BitZ Δ µs | PCS Δ µs | Root residual Δ µs |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | +132.52 | +115.58 | +106.46 | +96.90 | +1.43 | -8.29 | +13.09 | -2.84 |
| 512 | 128 | +226.82 | +83.69 | +61.09 | +28.15 | +1.76 | +81.26 | +24.55 | +18.65 |
| 1024 | 100 | +162.86 | +156.54 | +167.46 | +162.30 | -4.90 | +7.39 | -2.56 | -0.66 |
| 1024 | 128 | +32.58 | +60.90 | +76.32 | +60.70 | +1.15 | +6.07 | +0.70 | -23.61 |

Full absolute baseline/candidate durations and deltas for every recorded stage are in [component-deltas.json](component-deltas.json). [Stage summary](stage-summary.json), [metadata](metadata.json), [exact v7 identity audit](v7-reference-proof-identity.json), and [untraced identity audit](untraced-proof-identity.json) preserve the diagnosis. Raw stdout/stderr retain every proof and span.

The traced absolute deltas are substantially larger than the untraced production observations and must not be equated with that loss. The table below contrasts the 32-fresh-seed untraced check (pooled medians, seeds 106–137) with the single-input traced diagnosis (seed 42). Different inputs and trace overhead both contribute. These are context columns, not paired estimates of tracing cost.

| N | Security | Untraced v7 proof µs | Untraced R4 proof µs | Untraced Δ µs | Untraced Δ | Traced seed-42 root Δ µs |
|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 16372.83 | 16425.82 | +52.99 | +0.324% | +132.52 |
| 512 | 128 | 19989.76 | 20044.99 | +55.23 | +0.276% | +226.82 |
| 1024 | 100 | 22961.15 | 22999.97 | +38.83 | +0.169% | +162.86 |
| 1024 | 128 | 26651.76 | 26713.29 | +61.53 | +0.231% | +32.58 |

A separate static comparison found the raw binding algorithm's source files (`state.rs` and `streaming.rs`) byte-identical between v7 and R4. All 305 compared `BindingForm` machine-code functions had identical normalized instructions and sizes after address normalization; `State::fold` remained 28,612 bytes/5,479 instructions and `prepare` 16,696 bytes/3,242 instructions. Code placement changed (for example, the N512 fold's 64-byte offset changed from 32 to 16). These findings rule out additional raw-fold algorithm instructions as the explanation for this profile. Cache, allocation or placement effects remain possibilities, not established causes. The traced span differences alone cannot identify the mechanism.
