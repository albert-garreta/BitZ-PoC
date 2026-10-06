# Falcon one-source qualification

Status: **not_qualified**.

Each timing observation is the median of three measured proofs for one seed. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case.

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 60 | 0.9023 (0.9175) | 0.8692 (0.8859) | 0.8170 |
| native | 100 | 3 | 60 | 1.0196 (1.0383) | 0.8862 (0.9001) | 0.7651 |
| native | 100 | 32 | 43 | 0.9229 (0.9301) | 0.8293 (0.8366) | 0.7284 |

These completed pairs contain 1,304 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Roots and proof digests are expected to differ between revisions; input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 52.98 | 53.00 |
| native | 100 | 3 | 66.30 | 65.07 |
| native | 100 | 32 | 183.05 | 169.55 |

Stopped at a completed pair boundary after the full Native/100/batch3 prover cell failed to meet the 1.02 upper-confidence-bound gate (ratio 1.0196, upper 1.0383). Preserve every sample; V2 is incomplete and not qualified, and will not be combined with a revised candidate.
