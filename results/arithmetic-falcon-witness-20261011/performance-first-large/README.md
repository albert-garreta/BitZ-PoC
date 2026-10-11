# Performance-first large matrix

All 8 shapes pass both strict zero-tolerance proving gates. Proof-only medians improve 2.577–4.074%; total proving improves 1.618–2.915%. The plan used the fixed seeds 42–49, with one warmup and three measured proofs per variant/input. All 384 measured proofs and 128 warmups verified; paired input digests matched.

See the [full 24-shape report](../performance-first-report.md) for all medians, witness and payload sizes, mixed verifier changes, and measurement limits. The [strict gate](regression-gate.json), [paired-seed analysis](paired-seed-analysis.json), and [proof audit](reference-proof-identity.json) preserve the detailed evidence. The combined audit checks 768 exact Split proofs and 1,536 Single input/verification/physical-size matches across both matrices.
