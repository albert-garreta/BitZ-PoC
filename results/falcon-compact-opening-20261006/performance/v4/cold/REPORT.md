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
| native | 100 | 1 | 42.98 | 42.00 |
| native | 100 | 3 | 50.26 | 51.72 |
| native | 100 | 32 | 127.11 | 127.03 |
| native | 100 | 1024 | 2727.50 | 2725.98 |
| native | 128 | 1 | 42.77 | 43.94 |
| native | 128 | 3 | 49.21 | 49.70 |
| native | 128 | 32 | 126.27 | 129.05 |
| native | 128 | 1024 | 2724.08 | 2723.80 |
| shared-prime | 100 | 1 | 40.69 | 40.32 |
| shared-prime | 100 | 3 | 49.46 | 52.89 |
| shared-prime | 100 | 32 | 128.21 | 127.78 |
| shared-prime | 100 | 1024 | 2754.55 | 2756.83 |
| shared-prime | 128 | 1 | 43.17 | 42.21 |
| shared-prime | 128 | 3 | 51.16 | 50.14 |
| shared-prime | 128 | 32 | 127.72 | 127.58 |
| shared-prime | 128 | 1024 | 2753.19 | 2753.55 |
