Falcon speed integration qualification

Finite-sample comparison on this machine and these inputs only. Each seed contributes the median of repeated proofs in one process per variant. Reported 95% bootstrap intervals resample paired seed ratios, not individual proofs; three seeds give weak uncertainty estimates and do not establish nonregression. Missing fixture caches are primed in a separate untimed process before either paired measurement. RSS is process lifetime high-water memory, including fixture setup and warmups, not per-proof allocated memory. Preparation and fixture generation are excluded from total_prover_ms.

128 completed pairs across 16 cells. Ratios are candidate / baseline; lower is faster or uses less peak RSS.

| Mode | N | Batch | Security | Workers | Seeds | Total ratio | Proof ratio | Witness ratio | Verify ratio | Peak RSS ratio |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| arithmetic | 512 | 1 | 100 | 1 | 8 | 1.012 | 1.015 | 0.844 | 0.998 | 1.005 |
| arithmetic | 512 | 1 | 100 | 8 | 8 | 0.996 | 1.002 | 0.808 | 1.001 | 1.006 |
| arithmetic | 512 | 1 | 128 | 1 | 8 | 1.013 | 1.015 | 0.847 | 1.001 | 1.005 |
| arithmetic | 512 | 1 | 128 | 8 | 8 | 0.999 | 1.005 | 0.879 | 0.999 | 0.997 |
| arithmetic | 512 | 3 | 100 | 1 | 8 | 1.006 | 1.014 | 0.677 | 0.993 | 1.005 |
| arithmetic | 512 | 3 | 100 | 8 | 8 | 0.993 | 1.002 | 0.842 | 0.995 | 0.995 |
| arithmetic | 512 | 3 | 128 | 1 | 8 | 1.005 | 1.011 | 0.679 | 0.997 | 0.998 |
| arithmetic | 512 | 3 | 128 | 8 | 8 | 0.994 | 1.001 | 0.842 | 0.995 | 1.000 |
| arithmetic | 1024 | 1 | 100 | 1 | 8 | 1.007 | 1.015 | 0.640 | 0.996 | 1.006 |
| arithmetic | 1024 | 1 | 100 | 8 | 8 | 0.980 | 1.002 | 0.619 | 0.997 | 0.998 |
| arithmetic | 1024 | 1 | 128 | 1 | 8 | 1.003 | 1.010 | 0.636 | 0.999 | 1.002 |
| arithmetic | 1024 | 1 | 128 | 8 | 8 | 0.986 | 1.002 | 0.662 | 0.998 | 1.001 |
| arithmetic | 1024 | 3 | 100 | 1 | 8 | 0.989 | 1.011 | 0.423 | 0.984 | 0.999 |
| arithmetic | 1024 | 3 | 100 | 8 | 8 | 0.967 | 0.997 | 0.628 | 0.986 | 0.996 |
| arithmetic | 1024 | 3 | 128 | 1 | 8 | 0.990 | 1.008 | 0.423 | 0.988 | 0.995 |
| arithmetic | 1024 | 3 | 128 | 8 | 8 | 0.977 | 1.000 | 0.526 | 0.990 | 0.997 |

9 primary metric comparisons exceed the observed +1.0% review threshold. See confirmation-candidates.json; inspect seed ratios before choosing confirmation cells.
