# Production proof-layout check (R5)

**This revision fails the performance gate.** Fresh seeds 138–169 show positive paired-median and paired-geometric-mean 95% intervals for both proving metrics in all four unchanged batch-32, one-worker cases. Paired proof overhead is 0.76–1.38%, and paired total-prover overhead is 0.42–1.21%. Removing the remaining compact proof/module/types from production did not improve the observed result. The full 64-shape rerun was not started.

All **1,024 proofs verified** across **4 shapes**. Input digests matched for every pair. All **512 unchanged proofs** exactly matched the preserved v7 control; **0 proofs** used the reduced arithmetic layout and v8 transcript. Physical source bits, packed bytes and commitment roots remained unchanged for all inputs.

The shipping candidate enables arithmetic-padding reduction and uses the existing binding/bridge computation. Both variants use native release binaries with only `falcon-hybrid`. Failed experimental compact-computation pilots remain in the parent directory.

Each input had one warmup and three measured proofs, with 32 independent fresh seeds (138–169) for each of the four shapes. All timing processes ran sequentially with alternating variant order, pinned one/eight worker settings, and no concurrent builds, tests or analysis.

Positive percentages mean slower or larger. The table uses pooled-sample medians. `Proof paired 95%` is the seed-paired median bootstrap interval (10,000 resamples); intervals are descriptive and have no across-shape multiplicity adjustment. RSS is process peak memory, including preparation/external inputs.

| N | Security | Batch | Threads | Total Δ | Proof Δ | Proof paired 95% | Payload Δ | Verify Δ | RSS Δ |
|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|
| 512 | 100 | 32 | 1 | +1.16% | +1.45% | [+1.15%, +1.57%] | +0.00% | +0.35% | +0.19% |
| 512 | 128 | 32 | 1 | +1.00% | +1.17% | [+1.07%, +1.26%] | +0.00% | +0.24% | +0.13% |
| 1024 | 100 | 32 | 1 | +0.56% | +1.06% | [+0.78%, +1.23%] | +0.00% | +0.01% | -0.08% |
| 1024 | 128 | 32 | 1 | +0.53% | +0.71% | [+0.57%, +0.91%] | +0.00% | -0.10% | -0.26% |

The zero-threshold observed gate has **8 positive metric entries**. **8 entries** also have a positive lower endpoint in the descriptive paired-median interval. Positive observations remain in the evidence; they cannot be erased by gains in other shapes. Acceptance or confirmation decisions should consider both proving metrics and the complete paired analysis.

See [paired analysis](comparisons/padding-vs-v7/paired-seed-analysis.json), [observed gate](comparisons/padding-vs-v7/regression-gate.json), [layout audit](production-layout-audit.json), [exact v7 identity audit](v7-reference-proof-identity.json), and [run metadata](metadata.json). Raw per-input outputs, commands, sample timings and input hashes are preserved beside this report.
