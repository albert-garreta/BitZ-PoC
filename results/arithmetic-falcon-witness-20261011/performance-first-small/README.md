# Performance-first small matrix

All 16 shapes pass both strict zero-tolerance proving gates. Proof-only medians improve 0.213–3.558%; total proving improves 0.184–3.051%. The plan used the fixed seeds 42–73, with one warmup and three measured proofs per variant/input. All 3,072 measured proofs and 1,024 warmups verified; paired input digests matched.

See the [full 24-shape report](../performance-first-report.md) for all medians, witness and payload sizes, mixed verifier changes, and measurement limits. The [strict gate](regression-gate.json), [paired-seed analysis](paired-seed-analysis.json), and [proof audit](reference-proof-identity.json) preserve the detailed evidence. The combined audit checks 768 exact Split proofs and 1,536 Single input/verification/physical-size matches across both matrices.
