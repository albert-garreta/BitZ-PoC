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
| native | 100 | 1 | 46.64 | 42.59 |
| native | 100 | 3 | 59.22 | 49.45 |
| native | 100 | 32 | 133.66 | 126.63 |
| native | 100 | 1024 | 2980.01 | 2725.24 |
| native | 128 | 1 | 46.46 | 42.93 |
| native | 128 | 3 | 58.95 | 49.75 |
| native | 128 | 32 | 134.34 | 125.64 |
| native | 128 | 1024 | 3011.00 | 2723.60 |
| shared-prime | 100 | 1 | 45.58 | 42.24 |
| shared-prime | 100 | 3 | 55.71 | 50.13 |
| shared-prime | 100 | 32 | 134.85 | 125.47 |
| shared-prime | 100 | 1024 | 3005.98 | 2753.26 |
| shared-prime | 128 | 1 | 46.63 | 43.09 |
| shared-prime | 128 | 3 | 59.63 | 50.15 |
| shared-prime | 128 | 32 | 134.71 | 127.09 |
| shared-prime | 128 | 1024 | 3010.51 | 2751.58 |
