# Falcon one-source qualification

Status: **diagnostic**.

These diagnostics use one measured proof per process. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case.

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 1 | — | — | 0.8192 |
| native | 100 | 3 | 1 | — | — | 0.7642 |
| native | 100 | 32 | 1 | — | — | 0.7241 |
| native | 100 | 1024 | 1 | — | — | 0.7992 |
| native | 128 | 1 | 1 | — | — | 0.8426 |
| native | 128 | 3 | 1 | — | — | 0.7684 |
| native | 128 | 32 | 1 | — | — | 0.7289 |
| native | 128 | 1024 | 1 | — | — | 0.7855 |
| shared-prime | 100 | 1 | 1 | — | — | 0.8052 |
| shared-prime | 100 | 3 | 1 | — | — | 0.7620 |
| shared-prime | 100 | 32 | 1 | — | — | 0.7263 |
| shared-prime | 100 | 1024 | 1 | — | — | 0.7849 |
| shared-prime | 128 | 1 | 1 | — | — | 0.8214 |
| shared-prime | 128 | 3 | 1 | — | — | 0.7619 |
| shared-prime | 128 | 32 | 1 | — | — | 0.7293 |
| shared-prime | 128 | 1024 | 1 | — | — | 0.7899 |

These completed pairs contain 32 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Roots and proof digests are expected to differ between revisions; input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 45.20 | 41.90 |
| native | 100 | 3 | 58.43 | 56.16 |
| native | 100 | 32 | 132.70 | 124.88 |
| native | 100 | 1024 | 3011.49 | 2723.09 |
| native | 128 | 1 | 45.52 | 41.94 |
| native | 128 | 3 | 56.38 | 57.32 |
| native | 128 | 32 | 132.21 | 125.47 |
| native | 128 | 1024 | 3011.83 | 2724.44 |
| shared-prime | 100 | 1 | 44.87 | 41.72 |
| shared-prime | 100 | 3 | 53.89 | 53.83 |
| shared-prime | 100 | 32 | 135.39 | 125.96 |
| shared-prime | 100 | 1024 | 3005.66 | 2750.71 |
| shared-prime | 128 | 1 | 46.14 | 42.50 |
| shared-prime | 128 | 3 | 57.07 | 55.86 |
| shared-prime | 128 | 32 | 133.74 | 127.83 |
| shared-prime | 128 | 1024 | 3007.53 | 2751.25 |
