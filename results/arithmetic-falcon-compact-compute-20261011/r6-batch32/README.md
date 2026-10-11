# Selected strategy production focused check (R6)

**The focused check passes.** Fresh seeds 170–201 show negative paired-median and paired-geometric-mean 95% intervals for both proving metrics in all four unchanged batch-32, one-worker degree/security cases. Paired proof gains are 0.44–0.50% for N512 and 5.53–6.16% for N1024. This permits the preauthorized expanded 70-shape production qualification; it does not replace it.

All **1,024 proofs verified** across **4 shapes**. Input digests matched for every pair. All **512 unchanged proofs** exactly matched the preserved v7 control; **0 proofs** used the reduced arithmetic layout and v8 transcript. Physical source bits, packed bytes and commitment roots remained unchanged for all inputs.

The shipping candidate enables arithmetic-padding reduction and uses the existing binding/bridge computation. Both variants use native release binaries with only `falcon-hybrid`. Failed experimental compact-computation pilots remain in the parent directory.

Each input had one warmup and three measured proofs, with 32 independent fresh seeds (170–201) for each of the four shapes. All timing processes ran sequentially with alternating variant order, pinned one/eight worker settings, and no concurrent builds, tests or analysis.

Positive percentages mean slower or larger. The table uses pooled-sample medians. `Proof paired 95%` is the seed-paired median bootstrap interval (10,000 resamples); intervals are descriptive and have no across-shape multiplicity adjustment. RSS is process peak memory, including preparation/external inputs.

| N | Security | Batch | Threads | Total Δ | Proof Δ | Proof paired 95% | Payload Δ | Verify Δ | RSS Δ |
|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|
| 512 | 100 | 32 | 1 | -0.98% | -0.68% | [-0.64%, -0.36%] | +0.00% | -7.20% | +0.46% |
| 512 | 128 | 32 | 1 | -0.24% | -0.25% | [-0.54%, -0.39%] | +0.00% | -6.00% | -0.10% |
| 1024 | 100 | 32 | 1 | -5.93% | -6.16% | [-6.31%, -6.02%] | +0.00% | -10.37% | -0.23% |
| 1024 | 128 | 32 | 1 | -5.05% | -5.51% | [-5.61%, -5.24%] | +0.00% | -9.49% | +0.46% |

The zero-threshold observed gate has **0 positive metric entries**. **0 entries** also have a positive lower endpoint in the descriptive paired-median interval. Positive observations remain in the evidence; they cannot be erased by gains in other shapes. Acceptance or confirmation decisions should consider both proving metrics and the complete paired analysis.

See [paired analysis](comparisons/padding-vs-v7/paired-seed-analysis.json), [observed gate](comparisons/padding-vs-v7/regression-gate.json), [layout audit](production-layout-audit.json), [exact v7 identity audit](v7-reference-proof-identity.json), and [run metadata](metadata.json). Raw per-input outputs, commands, sample timings and input hashes are preserved beside this report.
