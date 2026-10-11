# Independent unchanged-layout confirmation

**The small proof-time regression repeats.** Fresh seeds 74–105 reproduce positive paired-median 95% intervals in all four unchanged batch-32, one-worker cases. Paired proof overhead is 0.35–0.60%; N512 total proving also has positive intervals. This rejects the current production binary under the no-repeatable-regression criterion.

All **1,024 proofs verified** across **4 shapes**. Input digests matched for every pair. All **512 unchanged proofs** exactly matched the preserved v7 control; **0 proofs** used the reduced arithmetic layout and v8 transcript. Physical source bits, packed bytes and commitment roots remained unchanged for all inputs.

The shipping candidate enables arithmetic-padding reduction and uses the existing binding/bridge computation. Both variants use native release binaries with only `falcon-hybrid`. Failed experimental compact-computation pilots remain in the parent directory.

Each input had one warmup and three measured proofs, with 32 independent fresh seeds (74–105) for each of the four shapes. All timing processes ran sequentially with alternating variant order, pinned one/eight worker settings, and no concurrent builds, tests or analysis.

Positive percentages mean slower or larger. The table uses pooled-sample medians. `Proof paired 95%` is the seed-paired median bootstrap interval (10,000 resamples); intervals are descriptive and have no across-shape multiplicity adjustment. RSS is process peak memory, including preparation/external inputs.

| N | Security | Batch | Threads | Total Δ | Proof Δ | Proof paired 95% | Payload Δ | Verify Δ | RSS Δ |
|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|
| 512 | 100 | 32 | 1 | +0.38% | +0.58% | [+0.44%, +0.66%] | +0.00% | +0.07% | +1.53% |
| 512 | 128 | 32 | 1 | +0.60% | +0.78% | [+0.40%, +0.66%] | +0.00% | -0.05% | +1.45% |
| 1024 | 100 | 32 | 1 | +0.36% | +0.49% | [+0.28%, +0.57%] | +0.00% | -0.27% | +1.33% |
| 1024 | 128 | 32 | 1 | +0.10% | +0.36% | [+0.17%, +0.48%] | +0.00% | -0.35% | +1.44% |

The zero-threshold observed gate has **8 positive metric entries**. **6 entries** also have a positive lower endpoint in the descriptive paired-median interval. Positive observations remain in the evidence; they cannot be erased by gains in other shapes. Acceptance or confirmation decisions should consider both proving metrics and the complete paired analysis.

See [paired analysis](comparisons/padding-vs-v7/paired-seed-analysis.json), [observed gate](comparisons/padding-vs-v7/regression-gate.json), [layout audit](production-layout-audit.json), [exact v7 identity audit](v7-reference-proof-identity.json), and [run metadata](metadata.json). Raw per-input outputs, commands, sample timings and input hashes are preserved beside this report.
