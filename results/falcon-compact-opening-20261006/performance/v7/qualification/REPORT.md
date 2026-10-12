# Falcon compact-opening qualification

Status: **not_qualified**.

Each timing observation is the median of three measured proofs for one seed. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case. Mean payload must be strictly smaller in every case. 

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 60 | 0.9846 (0.9958) | 0.9637 (0.9792) | 0.8683 |
| native | 100 | 3 | 60 | 0.9774 (1.0018) | 0.9768 (0.9950) | 0.8637 |
| native | 100 | 32 | 60 | 0.9902 (0.9962) | 0.9796 (0.9825) | 0.8938 |
| native | 100 | 1024 | 60 | 0.9881 (1.2884) | 0.9541 (1.0764) | 0.9461 |
| native | 128 | 1 | 2 | 1.0223 (1.0970) | 1.0483 (1.1328) | 0.8624 |

These completed pairs contain 1,936 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Source roots, security ledgers, and grinding diagnostics must match between revisions; stored proof digests may differ because the encoding changes. Input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Explicit CPU placement: worker indices map to CPUs `0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15`; the main thread is pinned to CPU 0. Requested and observed affinity masks must match in both revisions. This is a separately documented measurement environment; the fixed sample count and timing threshold are unchanged. The auxiliary pool retains 16 workers, pinned to CPU 0 for its serial Keccak fold job; all actual parallel work retains the global pool. The private flock verifier thread's singleton mask is checked after every trial outside elapsed timers.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 50.83 | 51.14 |
| native | 100 | 3 | 64.93 | 65.60 |
| native | 100 | 32 | 171.62 | 171.99 |
| native | 100 | 1024 | 3196.22 | 3205.93 |
| native | 128 | 1 | 47.96 | 51.21 |

Stopped after a complete 60-seed cell did not pass the unchanged compact-opening gate: {"case": ["native", 100, 1024], "payload_ratio": 0.9460701285594171, "payload_pass": true, "timings": {"total_prover_ms": {"ratio": 0.9880615453631453, "upper_one_sided_95": 1.2883862822229155, "status": "inconclusive"}, "proof_verify_ms": {"ratio": 0.9540896060451529, "upper_one_sided_95": 1.0764289016854425, "status": "inconclusive"}}}
