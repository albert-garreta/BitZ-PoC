# Falcon compact-opening qualification

Status: **not_qualified**.

Each timing observation is the median of three measured proofs for one seed. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case. Mean payload must be strictly smaller in every case. 

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 60 | 0.9956 (1.0048) | 0.9833 (0.9965) | 0.8683 |
| native | 100 | 3 | 60 | 1.0019 (1.0138) | 0.9943 (1.0132) | 0.8637 |
| native | 100 | 32 | 60 | 0.9944 (0.9978) | 0.9826 (0.9855) | 0.8938 |
| native | 100 | 1024 | 60 | 0.9978 (0.9988) | 0.9755 (0.9778) | 0.9461 |
| native | 128 | 1 | 60 | 1.0301 (1.0466) | 0.9907 (0.9936) | 0.8625 |
| native | 128 | 3 | 1 | — | — | 0.8586 |

These completed pairs contain 2,408 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Source roots, security ledgers, and grinding diagnostics must match between revisions; stored proof digests may differ because the encoding changes. Input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Explicit CPU placement: worker indices map to CPUs `0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15`; the main thread is pinned to CPU 0. Requested and observed affinity masks must match in both revisions. This is a separately documented measurement environment; the fixed sample count and timing threshold are unchanged. The auxiliary pool retains 16 workers, pinned to CPU 0 for its serial Keccak fold job; all actual parallel work retains the global pool. The private flock verifier thread's singleton mask is checked after every trial outside elapsed timers.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 52.66 | 51.62 |
| native | 100 | 3 | 65.77 | 64.79 |
| native | 100 | 32 | 170.94 | 172.10 |
| native | 100 | 1024 | 3135.46 | 3123.30 |
| native | 128 | 1 | 50.98 | 52.28 |
| native | 128 | 3 | 65.16 | 64.85 |

Stopped after a complete 60-seed cell did not pass the unchanged compact-opening gate: {"case": ["native", 128, 1], "payload_ratio": 0.8625362195329811, "payload_pass": true, "timings": {"total_prover_ms": {"ratio": 1.030148181098252, "upper_one_sided_95": 1.0466438104238078, "status": "inconclusive"}, "proof_verify_ms": {"ratio": 0.9907011062864811, "upper_one_sided_95": 0.9936468644843712, "status": "pass"}}}
