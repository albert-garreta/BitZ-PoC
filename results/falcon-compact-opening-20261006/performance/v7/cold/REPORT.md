# Falcon compact-opening qualification

Status: **diagnostic**.

These diagnostics use one measured proof per process. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case. Mean payload must be strictly smaller in every case. 

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 1 | — | — | 0.8683 |
| native | 100 | 3 | 1 | — | — | 0.8637 |
| native | 100 | 32 | 1 | — | — | 0.8927 |
| native | 100 | 1024 | 1 | — | — | 0.9459 |
| native | 128 | 1 | 1 | — | — | 0.8620 |
| native | 128 | 3 | 1 | — | — | 0.8586 |
| native | 128 | 32 | 1 | — | — | 0.8904 |
| native | 128 | 1024 | 1 | — | — | 0.9387 |
| shared-prime | 100 | 1 | 1 | — | — | 0.8675 |
| shared-prime | 100 | 3 | 1 | — | — | 0.8626 |
| shared-prime | 100 | 32 | 1 | — | — | 0.8927 |
| shared-prime | 100 | 1024 | 1 | — | — | 0.9408 |
| shared-prime | 128 | 1 | 1 | — | — | 0.8616 |
| shared-prime | 128 | 3 | 1 | — | — | 0.8596 |
| shared-prime | 128 | 32 | 1 | — | — | 0.8902 |
| shared-prime | 128 | 1024 | 1 | — | — | 0.9392 |

These completed pairs contain 32 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Source roots, security ledgers, and grinding diagnostics must match between revisions; stored proof digests may differ because the encoding changes. Input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Explicit CPU placement: worker indices map to CPUs `0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15`; the main thread is pinned to CPU 0. Requested and observed affinity masks must match in both revisions. This is a separately documented measurement environment; the fixed sample count and timing threshold are unchanged. The auxiliary pool retains 16 workers, pinned to CPU 0 for its serial Keccak fold job; all actual parallel work retains the global pool. The private flock verifier thread's singleton mask is checked after every trial outside elapsed timers.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 42.09 | 44.19 |
| native | 100 | 3 | 50.59 | 50.10 |
| native | 100 | 32 | 127.71 | 127.92 |
| native | 100 | 1024 | 2725.25 | 2724.69 |
| native | 128 | 1 | 44.40 | 43.08 |
| native | 128 | 3 | 49.72 | 50.57 |
| native | 128 | 32 | 126.89 | 127.30 |
| native | 128 | 1024 | 2725.64 | 2723.63 |
| shared-prime | 100 | 1 | 39.91 | 41.73 |
| shared-prime | 100 | 3 | 51.59 | 51.51 |
| shared-prime | 100 | 32 | 127.64 | 127.56 |
| shared-prime | 100 | 1024 | 2755.69 | 2754.21 |
| shared-prime | 128 | 1 | 43.30 | 43.41 |
| shared-prime | 128 | 3 | 50.27 | 50.81 |
| shared-prime | 128 | 32 | 128.16 | 127.95 |
| shared-prime | 128 | 1024 | 2753.69 | 2751.50 |
