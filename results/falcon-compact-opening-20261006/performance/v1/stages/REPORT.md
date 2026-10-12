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
| native | 100 | 1 | 42.06 | 42.02 |
| native | 100 | 3 | 49.19 | 48.34 |
| native | 100 | 32 | 125.42 | 124.59 |
| native | 100 | 1024 | 2719.89 | 2721.68 |
| native | 128 | 1 | 42.03 | 41.87 |
| native | 128 | 3 | 49.04 | 49.18 |
| native | 128 | 32 | 125.16 | 125.76 |
| native | 128 | 1024 | 2721.27 | 2721.80 |
| shared-prime | 100 | 1 | 41.36 | 41.59 |
| shared-prime | 100 | 3 | 50.11 | 50.38 |
| shared-prime | 100 | 32 | 124.01 | 126.26 |
| shared-prime | 100 | 1024 | 2749.62 | 2750.38 |
| shared-prime | 128 | 1 | 42.55 | 42.39 |
| shared-prime | 128 | 3 | 49.50 | 49.12 |
| shared-prime | 128 | 32 | 126.75 | 126.53 |
| shared-prime | 128 | 1024 | 2751.18 | 2752.41 |
