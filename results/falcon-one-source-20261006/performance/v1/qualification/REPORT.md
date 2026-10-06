# Falcon one-source qualification

Status: **not_qualified**.

Each timing observation is the median of three measured proofs for one seed. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case.

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 60 | 1.0107 (1.0261) | 0.9974 (1.0193) | 0.8170 |
| native | 100 | 3 | 60 | 1.0323 (1.0514) | 1.0050 (1.0149) | 0.7651 |
| native | 100 | 32 | 31 | 0.9717 (0.9840) | 0.9821 (0.9919) | 0.7286 |

These completed pairs contain 1,208 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Roots and proof digests are expected to differ between revisions; input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 52.87 | 52.59 |
| native | 100 | 3 | 66.82 | 67.84 |
| native | 100 | 32 | 180.68 | 175.36 |

Stopped at a complete pair boundary after a completed 60-seed cell could not satisfy the 1.02 upper-confidence-bound gate. No samples were discarded; this incomplete V1 campaign will not be combined with V2.
