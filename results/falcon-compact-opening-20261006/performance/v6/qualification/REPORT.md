# Falcon compact-opening qualification

Status: **not_qualified**.

Each timing observation is the median of three measured proofs for one seed. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case. Mean payload must be strictly smaller in every case. 

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 60 | 0.9870 (0.9971) | 0.9713 (0.9858) | 0.8683 |
| native | 100 | 3 | 60 | 0.9867 (1.0009) | 0.9668 (0.9862) | 0.8637 |
| native | 100 | 32 | 60 | 0.9968 (1.0008) | 0.9822 (0.9853) | 0.8938 |
| native | 100 | 1024 | 60 | 1.0001 (1.0023) | 0.9796 (0.9819) | 0.9461 |
| native | 128 | 1 | 60 | 1.0312 (1.0505) | 0.9825 (0.9863) | 0.8625 |
| native | 128 | 3 | 1 | — | — | 0.8586 |

These completed pairs contain 2,408 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Source roots, security ledgers, and grinding diagnostics must match between revisions; stored proof digests may differ because the encoding changes. Input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Explicit CPU placement: worker indices map to CPUs `0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15`; the main thread is pinned to CPU 0. Requested and observed affinity masks must match in both revisions. This is a separately documented measurement environment; the fixed sample count and timing threshold are unchanged. The auxiliary pool retains 16 workers, pinned to CPU 0 for its serial Keccak fold job; all actual parallel work retains the global pool. The private flock verifier thread's singleton mask is checked after every trial outside elapsed timers.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 51.37 | 52.77 |
| native | 100 | 3 | 66.46 | 66.01 |
| native | 100 | 32 | 170.83 | 174.51 |
| native | 100 | 1024 | 3128.04 | 3128.93 |
| native | 128 | 1 | 51.33 | 51.45 |
| native | 128 | 3 | 60.92 | 63.11 |

Resumed file monitor detected a completed cell failing the unchanged gate: [('native', 128, 1)]
