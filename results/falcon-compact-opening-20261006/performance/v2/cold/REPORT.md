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

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 42.54 | 42.36 |
| native | 100 | 3 | 49.55 | 49.62 |
| native | 100 | 32 | 125.49 | 126.24 |
| native | 100 | 1024 | 2723.79 | 2722.36 |
| native | 128 | 1 | 43.03 | 43.22 |
| native | 128 | 3 | 49.53 | 48.82 |
| native | 128 | 32 | 124.11 | 126.72 |
| native | 128 | 1024 | 2722.80 | 2723.04 |
| shared-prime | 100 | 1 | 41.70 | 42.12 |
| shared-prime | 100 | 3 | 50.62 | 49.66 |
| shared-prime | 100 | 32 | 126.24 | 124.59 |
| shared-prime | 100 | 1024 | 2749.80 | 2752.90 |
| shared-prime | 128 | 1 | 43.03 | 43.21 |
| shared-prime | 128 | 3 | 50.86 | 49.52 |
| shared-prime | 128 | 32 | 127.15 | 128.05 |
| shared-prime | 128 | 1024 | 2752.21 | 2748.50 |
