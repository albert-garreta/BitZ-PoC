# Falcon compact-opening qualification

Status: **not_qualified**.

Each timing observation is the median of three measured proofs for one seed. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case. Mean payload must be strictly smaller in every case. 

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 60 | 1.0016 (1.0186) | 1.0194 (1.0402) | 0.8683 |
| native | 100 | 3 | 18 | 1.0317 (1.0717) | 1.0347 (1.0756) | 0.8638 |

These completed pairs contain 624 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Source roots, security ledgers, and grinding diagnostics must match between revisions; stored proof digests may differ because the encoding changes. Input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 53.04 | 53.00 |
| native | 100 | 3 | 65.46 | 64.05 |

Stopped after a complete 60-seed cell did not pass the unchanged compact-opening gate: {"case": ["native", 100, 1], "payload_ratio": 0.8683006309366983, "payload_pass": true, "timings": {"total_prover_ms": {"ratio": 1.001609927570196, "upper_one_sided_95": 1.0185559237060013, "status": "pass"}, "proof_verify_ms": {"ratio": 1.019396110855221, "upper_one_sided_95": 1.0401951851320954, "status": "inconclusive"}}}
