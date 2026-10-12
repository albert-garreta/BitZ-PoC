# Falcon compact-opening qualification

Status: **not_qualified**.

Each timing observation is the median of three measured proofs for one seed. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case. Mean payload must be strictly smaller in every case. 

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 60 | 1.0177 (1.0283) | 1.0230 (1.0405) | 0.8683 |
| native | 100 | 3 | 1 | — | — | 0.8637 |

These completed pairs contain 488 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Source roots, security ledgers, and grinding diagnostics must match between revisions; stored proof digests may differ because the encoding changes. Input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Explicit CPU placement: worker indices map to CPUs `0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15`; the main thread is pinned to CPU 0. Requested and observed affinity masks must match in both revisions. This is a separately documented measurement environment; the fixed sample count and timing threshold are unchanged. The auxiliary pool retains 16 workers, pinned to CPU 0 for its serial Keccak fold job; all actual parallel work retains the global pool. The private flock verifier thread's singleton mask is checked after every trial outside elapsed timers.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 52.99 | 51.59 |
| native | 100 | 3 | 62.33 | 61.72 |

Stopped after a complete 60-seed cell did not pass the unchanged compact-opening gate: {"case": ["native", 100, 1], "payload_ratio": 0.8683006309366983, "payload_pass": true, "timings": {"total_prover_ms": {"ratio": 1.0177012612057297, "upper_one_sided_95": 1.0282768184679798, "status": "inconclusive"}, "proof_verify_ms": {"ratio": 1.0229630645901278, "upper_one_sided_95": 1.040531015300828, "status": "inconclusive"}}}
