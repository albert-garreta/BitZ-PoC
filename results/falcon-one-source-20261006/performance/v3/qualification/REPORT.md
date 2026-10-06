# Falcon one-source qualification

Status: **pass**.

Each timing observation is the median of three measured proofs for one seed. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case.

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 60 | 0.9022 (0.9178) | 0.8530 (0.8703) | 0.8170 |
| native | 100 | 3 | 60 | 0.8924 (0.9086) | 0.8400 (0.8562) | 0.7651 |
| native | 100 | 32 | 60 | 0.9185 (0.9287) | 0.8257 (0.8334) | 0.7286 |
| native | 100 | 1024 | 60 | 0.9521 (0.9571) | 0.9485 (0.9569) | 0.8028 |
| native | 128 | 1 | 60 | 0.9414 (0.9489) | 0.8156 (0.8258) | 0.8259 |
| native | 128 | 3 | 60 | 0.9032 (0.9102) | 0.7650 (0.7714) | 0.7672 |
| native | 128 | 32 | 60 | 0.9561 (0.9653) | 0.8404 (0.8461) | 0.7278 |
| native | 128 | 1024 | 60 | 0.9481 (0.9614) | 0.9399 (0.9473) | 0.7878 |
| shared-prime | 100 | 1 | 60 | 0.9155 (0.9332) | 0.9008 (0.9215) | 0.8148 |
| shared-prime | 100 | 3 | 60 | 0.9383 (0.9568) | 0.8130 (0.8259) | 0.7623 |
| shared-prime | 100 | 32 | 60 | 0.9220 (0.9286) | 0.8375 (0.8449) | 0.7266 |
| shared-prime | 100 | 1024 | 60 | 0.9485 (0.9561) | 0.9402 (0.9486) | 0.7875 |
| shared-prime | 128 | 1 | 60 | 0.9255 (0.9339) | 0.8371 (0.8447) | 0.8243 |
| shared-prime | 128 | 3 | 60 | 0.9191 (0.9254) | 0.7745 (0.7804) | 0.7669 |
| shared-prime | 128 | 32 | 60 | 0.9533 (0.9614) | 0.8491 (0.8550) | 0.7259 |
| shared-prime | 128 | 1024 | 60 | 0.9615 (0.9738) | 0.9469 (0.9518) | 0.7871 |

These completed pairs contain 7,680 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Roots and proof digests are expected to differ between revisions; input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 51.41 | 52.97 |
| native | 100 | 3 | 66.69 | 66.37 |
| native | 100 | 32 | 181.92 | 170.10 |
| native | 100 | 1024 | 3433.78 | 3171.38 |
| native | 128 | 1 | 52.41 | 50.95 |
| native | 128 | 3 | 68.42 | 64.32 |
| native | 128 | 32 | 183.28 | 170.20 |
| native | 128 | 1024 | 3431.25 | 3164.50 |
| shared-prime | 100 | 1 | 50.10 | 48.08 |
| shared-prime | 100 | 3 | 65.71 | 58.84 |
| shared-prime | 100 | 32 | 180.83 | 169.16 |
| shared-prime | 100 | 1024 | 3445.82 | 3198.74 |
| shared-prime | 128 | 1 | 51.39 | 49.45 |
| shared-prime | 128 | 3 | 69.61 | 66.55 |
| shared-prime | 128 | 32 | 183.48 | 169.61 |
| shared-prime | 128 | 1024 | 3443.38 | 3189.07 |
