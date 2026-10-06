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
| native | 100 | 1 | 46.52 | 42.30 |
| native | 100 | 3 | 58.11 | 56.47 |
| native | 100 | 32 | 131.72 | 126.24 |
| native | 100 | 1024 | 2978.15 | 2721.29 |
| native | 128 | 1 | 46.25 | 42.98 |
| native | 128 | 3 | 60.00 | 57.17 |
| native | 128 | 32 | 132.90 | 125.96 |
| native | 128 | 1024 | 3008.40 | 2724.18 |
| shared-prime | 100 | 1 | 45.05 | 42.07 |
| shared-prime | 100 | 3 | 54.43 | 53.27 |
| shared-prime | 100 | 32 | 135.95 | 126.75 |
| shared-prime | 100 | 1024 | 3006.48 | 2749.91 |
| shared-prime | 128 | 1 | 46.62 | 43.45 |
| shared-prime | 128 | 3 | 58.83 | 56.36 |
| shared-prime | 128 | 32 | 135.13 | 125.68 |
| shared-prime | 128 | 1024 | 3006.10 | 2750.60 |
