# R7 arithmetic Falcon production qualification

**R7 passes the requested proving-speed gate across all 70 measured shapes.** Proof-only and total-prover pooled medians improve in every shape against the preserved, previously qualified **v7 baseline**. There are **zero positive observations** and **zero confidence-positive cases** for either proving metric. All **280 descriptive paired 95% intervals**—70 shapes × two metrics × median/geometric-mean estimators—lie entirely below zero (negative means faster). No additional confirmation was needed.

All **7,552 proofs verified**, including 5,664 measured proofs and all warmups. Input digests matched for every pair. All **2,496 candidate proofs with unchanged arithmetic layouts exactly matched v7**, including full proof Debug digest, payload and source commitment. The other **1,280 candidate proofs** use reduced arithmetic capacity and the v8 transcript. Their physical source bits, packed bytes and commitment roots still match v7. There are 16 reduced-layout shapes and 54 unchanged-layout shapes.

The accepted implementation reduces arithmetic padding below the PCS allocation floor, selects the existing optimized raw-binding strategy/prefix by public shape, and groups byte-pair coefficient expansion with cap 16 on qualified Linux/x86_64 prefix-4 paths. The BitZ bridge protocol is unchanged for an unchanged arithmetic layout. Experimental compact-binding/multiple-bridge computation remains disabled in production; its failed measurements are preserved in the parent directory.

## Core proving gains

Positive percentages in this table mean faster. Each range spans both degrees, security targets and worker counts; it is a range of shape point estimates, not a confidence interval.

| Batch | Paired median proof gain | Pooled median proof gain | Paired median total gain | Pooled median total gain |
|---:|---:|---:|---:|---:|
| 1 | 18.70–28.70% | 18.51–28.75% | 17.85–28.43% | 17.62–28.64% |
| 32 | 5.33–16.25% | 5.57–16.21% | 4.97–14.28% | 4.88–14.54% |
| 1024 | 14.52–25.51% | 14.61–25.45% | 12.29–22.22% | 12.28–22.12% |

Across all 70 shapes, pooled proof gains are **2.84–31.28%** and pooled total-prover gains are **2.96–28.81%**. Paired median gains are **3.66–30.94%** for proof and **3.49–28.69%** for total. The smallest paired gains occur at N512/security128/batch17/eight workers; the largest occur at N512/security100/batch255/eight workers. Among the non-core edge shapes, pooled proof gains span 2.84–31.28% (minimum N512/security128/batch17/eight workers, maximum N1024/security100/batch129/eight workers). The weakest upper confidence endpoints are still −2.17% for proof paired median, −2.51% for proof geometric mean, −2.71% for total paired median, and −2.80% for total geometric mean.

## Witness size, payload and memory

For batch 1, the logical binding/arithmetic bit domain shrinks from 524,288 bits to 16,384 for N512 (**96.875% smaller**) or 32,768 for N1024 (**93.75% smaller**). At the half-floor edges, N512/batch16 and N1024/batch8, it halves. Physical PCS allocation, packed committed bytes and commitment roots remain unchanged. These are reductions in padded arithmetic work, not removal of the exact integer norm relation or a reduction of every committed witness buffer.

Reduced-layout payloads are **1.24–4.63% smaller** (1,056–4,408 bytes saved); all unchanged-layout payloads are identical to v7. Reduced-layout verifier medians improve by 1.99–13.22% and peak process RSS falls by 2.14–6.08%. Across the full matrix, verifier point estimates range from a 2.05% increase to a 44.71% decrease. Verifier times were recorded but are not the proving-speed acceptance metric.

Grouped computation trades some memory for speed. The largest median peak-RSS increase is **14.28%**, at N512/security100/batch1024/eight workers: 114,976→131,396 KiB (112.28→128.32 MiB). RSS measures the entire process, including external inputs and preparation. All per-shape payload, verifier and RSS values remain in the table and machine-readable summary.

## Reproducibility and scope

Both binaries use native release builds (`-C target-cpu=native`, default release LTO/profile) with **only `falcon-hybrid`**, run on **AMD Ryzen 9 9950X3D, Linux x86_64, one/eight workers**. One worker is pinned to CPU 6; eight workers use CPUs 6,7,4,5,2,3,0,1. The global and auxiliary pools use the same worker count. All processes run sequentially, with forward/reverse variant order balanced within each shape. There were no concurrent builds, tests, profiles or CPU-intensive analysis during timings.

The fixed [70-shape plan](../r7-qualification-plan.json) contains both degrees (512/1024), both security targets (100/128), core batches 1/32/1024, power-of-two edges and tails, and N512/eight-worker batches 255/256/257. Core batches 1/32 use seeds 42–73; all other shapes use seeds 42–49. Each input has one warmup and three measured proofs: 944 input cases per variant. This exactly repeats the preceding R6 method and adds no shape exclusions after observing outcomes.

Proof timing measures `prove`; total proving measures `commit_from_s2` plus `prove`. Reusable preparation, external Falcon input generation and HashToPoint, statement/coefficient cloning, input/proof hashing, RSS collection and diagnostics are outside these timers. External HashToPoint constructs benchmark inputs and is not in the arithmetic circuit. Verify timing is separate.

Bootstrap analysis uses **independent input seeds as paired units**, taking each seed's median over its three samples, with 10,000 fixed-PRNG resamples. The intervals are descriptive and have **no across-shape multiplicity correction**. The data establish no observed or confidence-positive proving regression in this measured scope; they do not establish a universal speed guarantee for other machines, targets, worker counts or inputs. Full Falcon is covered by correctness tests, not a full-Falcon performance benchmark here.

All **160 targeted correctness tests passed** before timing: 101 arithmetic, two explicit large cases, 54 shared opening/bridge/sumcheck cases, and three full Falcon cases. Build/test logs are in [validation](../validation/). The measured production build uses its normal release profile; test builds used native release with LTO disabled/codegen-units 16. The parent also completed review and a clean diff check.

The baseline binary SHA256 is `5208a4513f678c4bbd2ac0dafab83afda8971dd8fe9a4b634e371b9b80ea5273`; R7 is `ec1c7b55cfba4b7dbce26e4028913a13ccee6b87795f6b5ddaca58b6327ee141`. [R7 provenance](../candidate-r7-provenance.json) preserves its binary, 597-file build-input snapshot and hashes. The frozen source and binary were reverified after analysis. A later correction to one stale source comment is recorded separately in [post-build provenance](../candidate-r7-post-build-provenance.json) and its [exact patch](../candidate-r7-post-build-comment.patch); no executable source behavior changed. README updates are outside the build-input manifest.

## Full matrix

Positive percentages below mean slower or larger. Values are pooled-sample medians. `Proof paired 95%` is the seed-paired median bootstrap interval. Absolute values and all total-prover/median/geometric-mean intervals are retained in the linked machine-readable evidence.

| N | Security | Batch | Threads | Total Δ | Proof Δ | Proof paired 95% | Payload Δ | Verify Δ | RSS Δ |
|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|
| 512 | 100 | 1 | 1 | -24.11% | -24.36% | [-24.65%, -24.21%] | -4.63% | -12.97% | -5.27% |
| 512 | 100 | 1 | 8 | -19.16% | -20.20% | [-21.28%, -19.66%] | -4.63% | -12.73% | -5.49% |
| 512 | 100 | 32 | 1 | -12.05% | -12.57% | [-12.77%, -12.61%] | +0.00% | -7.35% | -0.36% |
| 512 | 100 | 32 | 8 | -8.27% | -8.37% | [-8.64%, -7.68%] | +0.00% | -14.31% | +0.01% |
| 512 | 128 | 1 | 1 | -28.64% | -28.75% | [-29.15%, -28.06%] | -4.40% | -13.16% | -6.08% |
| 512 | 128 | 1 | 8 | -26.60% | -27.16% | [-28.03%, -26.77%] | -4.40% | -13.22% | -5.02% |
| 512 | 128 | 32 | 1 | -10.09% | -10.46% | [-10.61%, -10.30%] | +0.00% | -6.12% | -0.11% |
| 512 | 128 | 32 | 8 | -4.88% | -5.57% | [-5.73%, -5.11%] | +0.00% | -11.93% | +0.38% |
| 1024 | 100 | 1 | 1 | -23.27% | -23.60% | [-24.08%, -23.22%] | -4.15% | -11.25% | -4.89% |
| 1024 | 100 | 1 | 8 | -17.62% | -18.51% | [-19.98%, -17.72%] | -4.15% | -11.41% | -4.77% |
| 1024 | 100 | 32 | 1 | -14.54% | -16.21% | [-16.35%, -16.05%] | +0.00% | -10.59% | +0.73% |
| 1024 | 100 | 32 | 8 | -13.54% | -15.86% | [-15.96%, -15.50%] | +0.00% | -22.69% | +0.72% |
| 1024 | 128 | 1 | 1 | -26.35% | -26.69% | [-27.12%, -26.08%] | -3.82% | -10.40% | -5.71% |
| 1024 | 128 | 1 | 8 | -24.14% | -24.89% | [-26.14%, -24.51%] | -3.82% | -10.05% | -4.53% |
| 1024 | 128 | 32 | 1 | -12.47% | -13.84% | [-14.25%, -13.73%] | +0.00% | -9.24% | +0.29% |
| 1024 | 128 | 32 | 8 | -9.59% | -10.82% | [-11.05%, -10.55%] | +0.00% | -19.75% | +0.58% |
| 512 | 100 | 1024 | 1 | -22.12% | -25.45% | [-25.62%, -25.23%] | +0.00% | +0.19% | +0.08% |
| 512 | 100 | 1024 | 8 | -14.80% | -15.85% | [-16.36%, -15.55%] | +0.00% | -0.49% | +14.28% |
| 512 | 128 | 1024 | 1 | -21.41% | -24.64% | [-24.91%, -24.40%] | +0.00% | +0.09% | +0.08% |
| 512 | 128 | 1024 | 8 | -15.83% | -17.29% | [-18.22%, -16.81%] | +0.00% | +0.79% | +14.15% |
| 1024 | 100 | 1024 | 1 | -12.38% | -15.25% | [-15.41%, -14.36%] | +0.00% | +0.46% | -0.05% |
| 1024 | 100 | 1024 | 8 | -12.92% | -14.61% | [-14.90%, -14.03%] | +0.00% | -0.16% | +0.05% |
| 1024 | 128 | 1024 | 1 | -12.28% | -14.93% | [-15.01%, -14.61%] | +0.00% | +0.08% | -0.09% |
| 1024 | 128 | 1024 | 8 | -14.66% | -16.92% | [-17.84%, -16.46%] | +0.00% | -0.58% | +0.02% |
| 512 | 100 | 16 | 1 | -9.44% | -9.85% | [-10.76%, -8.76%] | -1.61% | -1.99% | -2.98% |
| 512 | 100 | 16 | 8 | -5.83% | -6.00% | [-8.92%, -2.37%] | -1.61% | -2.99% | -2.47% |
| 512 | 100 | 17 | 1 | -9.89% | -10.34% | [-10.66%, -10.05%] | +0.00% | -6.31% | +1.03% |
| 512 | 100 | 17 | 8 | -4.99% | -5.52% | [-6.63%, -4.82%] | +0.00% | -10.18% | +0.25% |
| 512 | 100 | 33 | 1 | -15.48% | -16.27% | [-17.10%, -15.96%] | +0.00% | -10.14% | +0.08% |
| 512 | 100 | 33 | 8 | -13.05% | -14.06% | [-14.95%, -13.21%] | +0.00% | -17.78% | +1.14% |
| 512 | 100 | 65 | 1 | -18.89% | -20.36% | [-20.93%, -20.26%] | +0.00% | -15.10% | +0.62% |
| 512 | 100 | 65 | 8 | -19.09% | -21.46% | [-21.58%, -20.43%] | +0.00% | -27.68% | +0.43% |
| 512 | 100 | 129 | 1 | -22.60% | -24.34% | [-25.48%, -24.25%] | +0.00% | -20.97% | -0.18% |
| 512 | 100 | 129 | 8 | -26.33% | -28.28% | [-28.74%, -27.86%] | +0.00% | -40.01% | -3.47% |
| 512 | 128 | 16 | 1 | -10.50% | -10.78% | [-11.87%, -8.81%] | -1.36% | -5.06% | -2.14% |
| 512 | 128 | 16 | 8 | -14.59% | -15.23% | [-14.85%, -13.65%] | -1.36% | -5.63% | -2.74% |
| 512 | 128 | 17 | 1 | -7.81% | -8.00% | [-8.39%, -7.63%] | +0.00% | -5.52% | +1.80% |
| 512 | 128 | 17 | 8 | -2.96% | -2.84% | [-4.52%, -2.17%] | +0.00% | -8.12% | -0.06% |
| 512 | 128 | 33 | 1 | -12.56% | -13.22% | [-13.98%, -12.66%] | +0.00% | -8.95% | +0.56% |
| 512 | 128 | 33 | 8 | -8.80% | -9.23% | [-10.04%, -9.27%] | +0.00% | -14.90% | +1.13% |
| 512 | 128 | 65 | 1 | -17.62% | -18.37% | [-18.68%, -18.25%] | +0.00% | -13.42% | +0.11% |
| 512 | 128 | 65 | 8 | -14.09% | -15.40% | [-16.19%, -15.06%] | +0.00% | -24.86% | +0.47% |
| 512 | 128 | 129 | 1 | -20.74% | -22.24% | [-22.45%, -21.93%] | +0.00% | -20.18% | -0.09% |
| 512 | 128 | 129 | 8 | -21.54% | -22.23% | [-23.47%, -22.29%] | +0.00% | -36.98% | +0.75% |
| 1024 | 100 | 8 | 1 | -10.37% | -10.85% | [-11.00%, -10.40%] | -1.27% | -5.71% | -2.42% |
| 1024 | 100 | 8 | 8 | -8.59% | -8.93% | [-9.95%, -6.27%] | -1.27% | -5.51% | -2.32% |
| 1024 | 100 | 9 | 1 | -8.36% | -8.62% | [-8.88%, -8.21%] | +0.00% | -7.10% | +1.40% |
| 1024 | 100 | 9 | 8 | -7.49% | -8.06% | [-8.24%, -7.61%] | +0.00% | -9.92% | +0.73% |
| 1024 | 100 | 17 | 1 | -13.09% | -14.07% | [-14.31%, -13.69%] | +0.00% | -10.48% | +0.74% |
| 1024 | 100 | 17 | 8 | -11.88% | -13.17% | [-13.32%, -12.46%] | +0.00% | -17.33% | -0.91% |
| 1024 | 100 | 65 | 1 | -20.06% | -22.47% | [-22.55%, -22.25%] | +0.00% | -21.13% | +0.34% |
| 1024 | 100 | 65 | 8 | -24.41% | -26.17% | [-27.72%, -24.94%] | +0.00% | -39.95% | -0.87% |
| 1024 | 100 | 129 | 1 | -22.09% | -25.26% | [-25.27%, -25.09%] | +0.00% | -23.12% | +0.07% |
| 1024 | 100 | 129 | 8 | -28.81% | -31.28% | [-32.20%, -30.35%] | +0.00% | -44.71% | -0.55% |
| 1024 | 128 | 8 | 1 | -10.40% | -10.94% | [-11.14%, -9.96%] | -1.24% | -3.51% | -2.35% |
| 1024 | 128 | 8 | 8 | -15.05% | -14.74% | [-16.08%, -13.81%] | -1.24% | -3.99% | -2.23% |
| 1024 | 128 | 9 | 1 | -6.70% | -6.96% | [-7.69%, -7.25%] | +0.00% | -5.58% | -0.13% |
| 1024 | 128 | 9 | 8 | -4.75% | -4.90% | [-6.57%, -4.05%] | +0.00% | -8.19% | +0.36% |
| 1024 | 128 | 17 | 1 | -11.45% | -12.14% | [-12.51%, -11.94%] | +0.00% | -8.99% | -0.39% |
| 1024 | 128 | 17 | 8 | -7.80% | -8.65% | [-9.05%, -8.48%] | +0.00% | -14.49% | +0.39% |
| 1024 | 128 | 65 | 1 | -18.93% | -20.69% | [-20.97%, -20.22%] | +0.00% | -20.28% | -0.87% |
| 1024 | 128 | 65 | 8 | -17.94% | -20.99% | [-22.30%, -19.22%] | +0.00% | -37.42% | +0.14% |
| 1024 | 128 | 129 | 1 | -20.98% | -23.52% | [-23.89%, -23.30%] | +0.00% | -22.18% | -0.82% |
| 1024 | 128 | 129 | 8 | -25.15% | -27.59% | [-28.21%, -27.21%] | +0.00% | -42.33% | -0.02% |
| 512 | 100 | 255 | 8 | -28.55% | -31.04% | [-31.93%, -30.62%] | +0.00% | -42.22% | +0.63% |
| 512 | 100 | 256 | 8 | -12.24% | -13.35% | [-13.34%, -12.09%] | +0.00% | -0.40% | +1.10% |
| 512 | 100 | 257 | 8 | -14.03% | -15.27% | [-15.73%, -14.82%] | +0.00% | +2.05% | -0.19% |
| 512 | 128 | 255 | 8 | -24.00% | -25.94% | [-26.50%, -25.29%] | +0.00% | -39.36% | +0.29% |
| 512 | 128 | 256 | 8 | -11.16% | -11.72% | [-12.47%, -10.93%] | +0.00% | -1.57% | +0.73% |
| 512 | 128 | 257 | 8 | -11.99% | -13.23% | [-13.69%, -12.73%] | +0.00% | +0.87% | +0.20% |

The [qualification summary](qualification-summary.json) records exact core/edge ranges and all confidence counts. See [paired analysis](comparisons/padding-vs-v7/paired-seed-analysis.json), [empty confidence-positive union](confidence-positive-union.json), [observed gate](comparisons/padding-vs-v7/regression-gate.json), [layout audit](production-layout-audit.json), [exact v7 identity audit](v7-reference-proof-identity.json), and [run metadata](metadata.json). Raw per-input outputs, exact commands, sample timings and input hashes are preserved beside this report. Failed prior candidates remain in the [parent report](../README.md); R7 acceptance does not reinterpret their regressions.
