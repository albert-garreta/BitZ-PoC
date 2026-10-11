# Qualified v7 small matrix

This fixed 32-seed matrix does **not pass** the zero-tolerance gate. N512, batch 32, one thread has total/proof deltas +0.3348%/+0.0425% at security 100 and +0.0823%/+0.0431% at security 128. The other fourteen shapes improve both pooled medians. These tiny proof deltas are not established regressions; the gate remains failed.

All 3,072 measured proofs and 1,024 warmups verified. Seeds 42–73 were selected before running; each input and variant had one warmup and three measurements. Every paired input digest matched. The combined identity audit verifies all 768 shared proofs across the small and large matrices.

See the [full 24-shape report](../v7-qualified-report.md), [strict timing gate](regression-gate.json), [paired-seed statistics](paired-seed-analysis.json), and [combined proof audit](../v7-qualified-proof-identity.json). The report preserves these results before the subsequent minimum-size layout fallback.
