# Selected strategy production qualification (R6)

**R6 is not accepted as regression-free.** It improves both pooled proving medians in 61/70 shapes, including strong small/medium gains, but six shapes have a positive lower endpoint in at least one paired-median or paired-geometric-mean 95% interval. These concern unchanged large/boundary layouts; all failures remain recorded. No independent R6 confirmation was run: root chose a separate grouped-computation experiment before spending more time confirming a candidate that would not yet be accepted.

All **7,552 proofs verified** across **70 shapes**. Input digests matched for every pair. All **2,496 unchanged proofs** exactly matched the preserved v7 control; **1,280 proofs** used the reduced arithmetic layout and v8 transcript. Physical source bits, packed bytes and commitment roots remained unchanged for all inputs.

The candidate enables arithmetic-padding reduction and the selected public-threshold raw-binding strategy; compact binding/bridge computation stays disabled in production. Both variants use native release binaries with only `falcon-hybrid`. Failed experimental compact-computation pilots remain in the parent directory.

Each input had one warmup and three measured proofs, with 32 independent seeds for core-small shapes and eight for core-large and edge shapes. All timing processes ran sequentially with alternating variant order, pinned one/eight worker settings, and no concurrent builds, tests or analysis.

Positive percentages mean slower or larger. The table uses pooled-sample medians. `Proof paired 95%` is the seed-paired median bootstrap interval (10,000 resamples); intervals are descriptive and have no across-shape multiplicity adjustment. RSS is process peak memory, including preparation/external inputs.

| N | Security | Batch | Threads | Total Δ | Proof Δ | Proof paired 95% | Payload Δ | Verify Δ | RSS Δ |
|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|
| 512 | 100 | 1 | 1 | -23.88% | -24.14% | [-24.66%, -23.93%] | -4.63% | -12.69% | -5.29% |
| 512 | 100 | 1 | 8 | -19.35% | -20.05% | [-21.25%, -19.19%] | -4.63% | -12.71% | -5.44% |
| 512 | 100 | 32 | 1 | -0.45% | -0.47% | [-0.57%, -0.30%] | +0.00% | -7.29% | +0.84% |
| 512 | 100 | 32 | 8 | -7.44% | -8.09% | [-9.15%, -7.90%] | +0.00% | -14.14% | -0.22% |
| 512 | 128 | 1 | 1 | -28.61% | -28.85% | [-29.38%, -28.29%] | -4.40% | -13.19% | -6.17% |
| 512 | 128 | 1 | 8 | -26.69% | -27.30% | [-28.65%, -27.05%] | -4.40% | -13.14% | -5.15% |
| 512 | 128 | 32 | 1 | -0.48% | -0.36% | [-0.49%, -0.29%] | +0.00% | -6.18% | +0.17% |
| 512 | 128 | 32 | 8 | -5.06% | -5.50% | [-5.41%, -4.91%] | +0.00% | -11.60% | +0.22% |
| 1024 | 100 | 1 | 1 | -23.02% | -23.36% | [-23.71%, -23.22%] | -4.15% | -11.24% | -4.87% |
| 1024 | 100 | 1 | 8 | -17.86% | -18.70% | [-19.67%, -17.32%] | -4.15% | -11.30% | -4.42% |
| 1024 | 100 | 32 | 1 | -6.06% | -6.44% | [-6.60%, -6.29%] | +0.00% | -10.33% | +0.00% |
| 1024 | 100 | 32 | 8 | -12.66% | -13.90% | [-14.63%, -13.71%] | +0.00% | -22.48% | +0.88% |
| 1024 | 128 | 1 | 1 | -26.43% | -26.76% | [-26.99%, -25.82%] | -3.82% | -10.00% | -5.81% |
| 1024 | 128 | 1 | 8 | -24.29% | -24.97% | [-26.76%, -24.74%] | -3.82% | -10.44% | -4.67% |
| 1024 | 128 | 32 | 1 | -4.89% | -5.39% | [-5.50%, -5.32%] | +0.00% | -9.29% | +0.72% |
| 1024 | 128 | 32 | 8 | -9.14% | -10.36% | [-10.85%, -10.33%] | +0.00% | -19.59% | +0.38% |
| 512 | 100 | 1024 | 1 | +0.42% | +0.65% | [+0.52%, +1.05%] | +0.00% | +0.46% | +0.05% |
| 512 | 100 | 1024 | 8 | +0.25% | +0.42% | [-0.56%, +1.73%] | +0.00% | +1.05% | +0.05% |
| 512 | 128 | 1024 | 1 | -0.04% | +0.35% | [-0.22%, +0.93%] | +0.00% | +0.12% | -0.07% |
| 512 | 128 | 1024 | 8 | +0.42% | +0.87% | [+0.06%, +1.46%] | +0.00% | +1.43% | +0.09% |
| 1024 | 100 | 1024 | 1 | -0.48% | -0.02% | [-0.46%, +0.24%] | +0.00% | +0.25% | -0.10% |
| 1024 | 100 | 1024 | 8 | -4.92% | -5.40% | [-6.40%, -4.86%] | +0.00% | +1.83% | +0.02% |
| 1024 | 128 | 1024 | 1 | -0.15% | +0.23% | [-0.04%, +0.40%] | +0.00% | +0.44% | +0.00% |
| 1024 | 128 | 1024 | 8 | -5.78% | -6.53% | [-7.13%, -6.04%] | +0.00% | +1.34% | -0.08% |
| 512 | 100 | 16 | 1 | -9.31% | -9.71% | [-10.59%, -9.43%] | -1.61% | -1.33% | -2.70% |
| 512 | 100 | 16 | 8 | -5.21% | -5.35% | [-8.64%, -1.66%] | -1.61% | -1.60% | -2.57% |
| 512 | 100 | 17 | 1 | -3.03% | -3.20% | [-3.73%, -2.39%] | +0.00% | -7.17% | +0.07% |
| 512 | 100 | 17 | 8 | -4.69% | -5.47% | [-5.98%, -3.87%] | +0.00% | -9.97% | +0.50% |
| 512 | 100 | 33 | 1 | -5.75% | -5.78% | [-6.46%, -5.42%] | +0.00% | -10.31% | +0.32% |
| 512 | 100 | 33 | 8 | -9.32% | -10.63% | [-11.72%, -10.04%] | +0.00% | -16.89% | +0.79% |
| 512 | 100 | 65 | 1 | -7.15% | -7.42% | [-7.76%, -7.11%] | +0.00% | -14.68% | +0.68% |
| 512 | 100 | 65 | 8 | -14.42% | -16.29% | [-16.97%, -15.30%] | +0.00% | -27.49% | +0.30% |
| 512 | 100 | 129 | 1 | -8.21% | -8.78% | [-9.00%, -8.64%] | +0.00% | -21.10% | -0.96% |
| 512 | 100 | 129 | 8 | -19.38% | -20.67% | [-23.19%, -20.48%] | +0.00% | -39.61% | -2.19% |
| 512 | 128 | 16 | 1 | -10.51% | -10.74% | [-12.14%, -8.38%] | -1.36% | -4.48% | -2.69% |
| 512 | 128 | 16 | 8 | -14.79% | -15.06% | [-15.51%, -14.12%] | -1.36% | -5.63% | -2.56% |
| 512 | 128 | 17 | 1 | -2.72% | -2.83% | [-3.12%, -2.57%] | +0.00% | -5.40% | -0.35% |
| 512 | 128 | 17 | 8 | -3.21% | -3.19% | [-4.36%, -1.95%] | +0.00% | -8.20% | -0.09% |
| 512 | 128 | 33 | 1 | -4.49% | -4.84% | [-4.99%, -4.12%] | +0.00% | -9.19% | +0.57% |
| 512 | 128 | 33 | 8 | -7.49% | -7.49% | [-8.12%, -7.49%] | +0.00% | -14.99% | +0.71% |
| 512 | 128 | 65 | 1 | -5.57% | -5.96% | [-7.03%, -6.08%] | +0.00% | -13.40% | -1.33% |
| 512 | 128 | 65 | 8 | -10.85% | -11.85% | [-12.24%, -11.94%] | +0.00% | -24.17% | -0.06% |
| 512 | 128 | 129 | 1 | -7.56% | -8.16% | [-8.48%, -7.86%] | +0.00% | -19.61% | +0.00% |
| 512 | 128 | 129 | 8 | -15.04% | -15.95% | [-16.34%, -15.94%] | +0.00% | -36.29% | +1.08% |
| 1024 | 100 | 8 | 1 | -10.05% | -10.62% | [-10.93%, -10.34%] | -1.27% | -5.36% | -2.37% |
| 1024 | 100 | 8 | 8 | -8.98% | -8.90% | [-10.13%, -7.67%] | -1.27% | -5.81% | -3.12% |
| 1024 | 100 | 9 | 1 | -4.58% | -4.87% | [-5.16%, -4.82%] | +0.00% | -6.61% | -0.09% |
| 1024 | 100 | 9 | 8 | -6.34% | -7.17% | [-7.73%, -6.65%] | +0.00% | -9.75% | +0.34% |
| 1024 | 100 | 17 | 1 | -7.64% | -8.16% | [-8.39%, -7.92%] | +0.00% | -10.65% | +0.30% |
| 1024 | 100 | 17 | 8 | -10.67% | -12.49% | [-12.84%, -11.85%] | +0.00% | -16.95% | -0.21% |
| 1024 | 100 | 65 | 1 | -11.38% | -12.66% | [-13.43%, -12.52%] | +0.00% | -21.03% | +0.38% |
| 1024 | 100 | 65 | 8 | -18.83% | -21.73% | [-23.29%, -21.31%] | +0.00% | -39.61% | -0.78% |
| 1024 | 100 | 129 | 1 | -13.04% | -14.75% | [-15.00%, -14.57%] | +0.00% | -22.71% | -0.12% |
| 1024 | 100 | 129 | 8 | -24.38% | -26.43% | [-27.40%, -25.93%] | +0.00% | -44.33% | +0.02% |
| 1024 | 128 | 8 | 1 | -10.53% | -10.86% | [-11.19%, -9.79%] | -1.24% | -3.38% | -2.59% |
| 1024 | 128 | 8 | 8 | -14.83% | -14.95% | [-16.60%, -14.12%] | -1.24% | -4.16% | -1.91% |
| 1024 | 128 | 9 | 1 | -4.25% | -3.90% | [-4.70%, -3.83%] | +0.00% | -5.64% | +0.09% |
| 1024 | 128 | 9 | 8 | -4.89% | -4.99% | [-6.41%, -4.32%] | +0.00% | -8.10% | +0.04% |
| 1024 | 128 | 17 | 1 | -6.80% | -6.95% | [-7.41%, -6.84%] | +0.00% | -9.03% | +0.40% |
| 1024 | 128 | 17 | 8 | -7.72% | -8.59% | [-8.94%, -8.01%] | +0.00% | -14.58% | +0.49% |
| 1024 | 128 | 65 | 1 | -11.06% | -12.20% | [-12.46%, -11.87%] | +0.00% | -20.30% | -0.76% |
| 1024 | 128 | 65 | 8 | -17.11% | -18.41% | [-19.55%, -16.04%] | +0.00% | -36.89% | -0.62% |
| 1024 | 128 | 129 | 1 | -12.51% | -14.02% | [-14.37%, -13.74%] | +0.00% | -22.07% | -1.08% |
| 1024 | 128 | 129 | 8 | -21.48% | -23.55% | [-23.90%, -22.54%] | +0.00% | -42.14% | +0.20% |
| 512 | 100 | 255 | 8 | -20.47% | -21.97% | [-22.57%, -21.09%] | +0.00% | -40.76% | +0.05% |
| 512 | 100 | 256 | 8 | +1.80% | +1.23% | [-0.25%, +2.22%] | +0.00% | +1.15% | +0.36% |
| 512 | 100 | 257 | 8 | +0.07% | +0.31% | [-0.93%, +1.16%] | +0.00% | +0.82% | +0.10% |
| 512 | 128 | 255 | 8 | -17.69% | -18.75% | [-19.13%, -17.93%] | +0.00% | -39.95% | +0.52% |
| 512 | 128 | 256 | 8 | +1.05% | +0.99% | [+0.59%, +1.51%] | +0.00% | +0.31% | +0.00% |
| 512 | 128 | 257 | 8 | +0.79% | +0.86% | [-0.12%, +1.16%] | +0.00% | +0.50% | -0.08% |

The zero-threshold observed gate has **16 positive metric entries**. **5 entries** also have a positive lower endpoint in the descriptive paired-median interval. Positive observations remain in the evidence; they cannot be erased by gains in other shapes. Acceptance or confirmation decisions should consider both proving metrics and the complete paired analysis.

See [paired analysis](comparisons/padding-vs-v7/paired-seed-analysis.json), [observed gate](comparisons/padding-vs-v7/regression-gate.json), [layout audit](production-layout-audit.json), [exact v7 identity audit](v7-reference-proof-identity.json), and [run metadata](metadata.json). Raw per-input outputs, commands, sample timings and input hashes are preserved beside this report.

The fixed expanded plan preserves all original 64 shapes/seeds and adds N512/eight-worker batches255/256/257 at both security levels, with eight seeds each. These additions check the selected prefix boundary at256. See [expanded plan](../r6-qualification-plan.json) and [R6 frozen provenance](../candidate-r6-provenance.json).

The full union of positive paired-median/geometric-mean interval cases is in [confidence-positive-union.json](confidence-positive-union.json). It contains N512/security100/batch1024/one worker, N512/security128/batch1024/eight workers, N1024/security128/batch1024/one worker, N512/batch256/eight workers at both security targets, and N512/security128/batch257/eight workers. Some are geometric-mean-only flags; the complete intervals distinguish them. These descriptive intervals do not have across-shape multiplicity correction.
