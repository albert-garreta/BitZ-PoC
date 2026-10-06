# Falcon one-source qualification

Status: **diagnostic**.

Each timing observation is the median of three measured proofs for one seed. Ratios compare arithmetic means across seeds. The upper confidence bound is a paired one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and both timing bounds at or below 1.02 in every case.

| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |
|---|---:|---:|---:|---:|---:|---:|
| native | 100 | 1 | 2 | 1.0026 (1.0497) | 1.0330 (1.0721) | 0.8204 |
| native | 100 | 3 | 2 | 1.0454 (1.1241) | 1.0112 (1.0536) | 0.7615 |
| native | 100 | 32 | 2 | 0.9692 (0.9695) | 0.9694 (0.9828) | 0.7264 |
| native | 100 | 1024 | 2 | 0.9372 (0.9429) | 1.0185 (1.0200) | 0.8000 |
| native | 128 | 1 | 2 | 1.0271 (1.0685) | 1.0061 (1.0332) | 0.8326 |
| native | 128 | 3 | 2 | 1.0056 (1.0269) | 0.9971 (1.0102) | 0.7656 |
| native | 128 | 32 | 2 | 0.9881 (1.0185) | 1.0086 (1.0296) | 0.7281 |
| native | 128 | 1024 | 2 | 0.9924 (1.0041) | 0.9883 (0.9954) | 0.7864 |
| shared-prime | 100 | 1 | 2 | 1.1742 (1.1835) | 1.0706 (1.1393) | 0.8102 |
| shared-prime | 100 | 3 | 2 | 0.9810 (1.0171) | 1.0339 (1.1565) | 0.7640 |
| shared-prime | 100 | 32 | 2 | 0.9191 (0.9262) | 1.0074 (1.0417) | 0.7255 |
| shared-prime | 100 | 1024 | 2 | 0.9470 (0.9508) | 1.0148 (1.0320) | 0.7870 |
| shared-prime | 128 | 1 | 2 | 0.9770 (0.9882) | 0.9785 (0.9811) | 0.8235 |
| shared-prime | 128 | 3 | 2 | 0.9954 (1.0483) | 0.9156 (0.9238) | 0.7671 |
| shared-prime | 128 | 32 | 2 | 0.9990 (1.0305) | 0.9519 (0.9590) | 0.7286 |
| shared-prime | 128 | 1024 | 2 | 1.0295 (1.0336) | 1.0325 (1.0766) | 0.7859 |

These completed pairs contain 256 verified proofs including warmups. A complete qualification requires 1,920 separate processes and 7,680 proofs. Roots and proof digests are expected to differ between revisions; input digests, public statements, security targets, geometry, compiler settings, and arithmetic profiles must match. A diagnostic or incomplete campaign cannot satisfy the gate.

Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.

| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |
|---|---:|---:|---:|---:|
| native | 100 | 1 | 50.43 | 50.52 |
| native | 100 | 3 | 63.22 | 63.48 |
| native | 100 | 32 | 180.01 | 172.24 |
| native | 100 | 1024 | 3393.42 | 3146.70 |
| native | 128 | 1 | 51.05 | 50.68 |
| native | 128 | 3 | 66.83 | 63.77 |
| native | 128 | 32 | 181.53 | 170.32 |
| native | 128 | 1024 | 3381.61 | 3126.00 |
| shared-prime | 100 | 1 | 49.73 | 49.57 |
| shared-prime | 100 | 3 | 63.66 | 63.56 |
| shared-prime | 100 | 32 | 179.91 | 172.27 |
| shared-prime | 100 | 1024 | 3419.54 | 3167.84 |
| shared-prime | 128 | 1 | 51.46 | 50.27 |
| shared-prime | 128 | 3 | 64.99 | 65.05 |
| shared-prime | 128 | 32 | 183.19 | 172.40 |
| shared-prime | 128 | 1024 | 3433.83 | 3181.45 |
