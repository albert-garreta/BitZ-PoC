# Revised production focused check (R4)

**This revision still does not pass the proof-time gate.** Compile-time exclusion of compact computation lowers the earlier overhead, but fresh seeds 106–137 retain positive proof-time paired-median intervals for N512/security100, N512/security128 and N1024/security100. Paired proof medians are about +0.24–0.25%; N1024/security128 is inconclusive around +0.02%. Total-prover intervals no longer show a positive lower bound. The full 64-shape rerun was not started.

All **1,024 proofs verified** across **4 shapes**. Input digests matched for every pair. All **512 unchanged proofs** exactly matched the preserved v7 control; **0 proofs** used the reduced arithmetic layout and v8 transcript. Physical source bits, packed bytes and commitment roots remained unchanged for all inputs.

The shipping candidate enables arithmetic-padding reduction and uses the existing binding/bridge computation. Both variants use native release binaries with only `falcon-hybrid`. Failed experimental compact-computation pilots remain in the parent directory.

Each input had one warmup and three measured proofs, with 32 independent fresh seeds (106–137) for each of the four shapes. All timing processes ran sequentially with alternating variant order, pinned one/eight worker settings, and no concurrent builds, tests or analysis.

Positive percentages mean slower or larger. The table uses pooled-sample medians. `Proof paired 95%` is the seed-paired median bootstrap interval (10,000 resamples); intervals are descriptive and have no across-shape multiplicity adjustment. RSS is process peak memory, including preparation/external inputs.

| N | Security | Batch | Threads | Total Δ | Proof Δ | Proof paired 95% | Payload Δ | Verify Δ | RSS Δ |
|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|
| 512 | 100 | 32 | 1 | -0.04% | +0.32% | [+0.13%, +0.42%] | +0.00% | +0.05% | +0.17% |
| 512 | 128 | 32 | 1 | +0.14% | +0.28% | [+0.12%, +0.40%] | +0.00% | +0.19% | +0.45% |
| 1024 | 100 | 32 | 1 | -0.05% | +0.17% | [+0.07%, +0.47%] | +0.00% | +0.02% | -0.03% |
| 1024 | 128 | 32 | 1 | +0.00% | +0.23% | [-0.13%, +0.15%] | +0.00% | +0.01% | -0.44% |

The zero-threshold observed gate has **6 positive metric entries**. **3 entries** also have a positive lower endpoint in the descriptive paired-median interval. Positive observations remain in the evidence; they cannot be erased by gains in other shapes. Acceptance or confirmation decisions should consider both proving metrics and the complete paired analysis.

See [paired analysis](comparisons/padding-vs-v7/paired-seed-analysis.json), [observed gate](comparisons/padding-vs-v7/regression-gate.json), [layout audit](production-layout-audit.json), [exact v7 identity audit](v7-reference-proof-identity.json), and [run metadata](metadata.json). Raw per-input outputs, commands, sample timings and input hashes are preserved beside this report.
