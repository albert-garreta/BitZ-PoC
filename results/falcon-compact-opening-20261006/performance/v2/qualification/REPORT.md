# Falcon compact-opening qualification

Status: **not_qualified**.

Each timing observation is the median of three measured proofs for one seed. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case. Mean payload must be strictly smaller in every case. 

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 60 | 0.9921 (1.0039) | 0.9838 (1.0037) | 0.8683 |
| native | 100 | 3 | 60 | 1.0027 (1.0195) | 1.0060 (1.0277) | 0.8637 |
| native | 100 | 32 | 2 | 0.9934 (0.9956) | 1.0174 (1.0244) | 0.8933 |

These completed pairs contain 976 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Source roots, security ledgers, and grinding diagnostics must match between revisions; stored proof digests may differ because the encoding changes. Input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 51.19 | 49.42 |
| native | 100 | 3 | 66.36 | 65.98 |
| native | 100 | 32 | 167.96 | 168.57 |

Stopped after a complete 60-seed cell did not pass the unchanged compact-opening gate: {"case": ["native", 100, 3], "payload_ratio": 0.8637139381278457, "payload_pass": true, "timings": {"total_prover_ms": {"ratio": 1.002708590422063, "upper_one_sided_95": 1.0194963377120603, "status": "pass"}, "proof_verify_ms": {"ratio": 1.0060164678064067, "upper_one_sided_95": 1.027708189167554, "status": "inconclusive"}}}
