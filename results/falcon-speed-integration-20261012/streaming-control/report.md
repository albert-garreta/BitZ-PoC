Falcon speed integration qualification

Finite-sample comparison on this machine and these inputs only. Each seed contributes the median of repeated proofs in one process per variant. Reported 95% bootstrap intervals resample paired seed ratios, not individual proofs; three seeds give weak uncertainty estimates and do not establish nonregression. Missing fixture caches are primed in a separate untimed process before either paired measurement. RSS is process lifetime high-water memory, including fixture setup and warmups, not per-proof allocated memory. Preparation and fixture generation are excluded from total_prover_ms.

64 completed pairs across 8 cells. Ratios are candidate / baseline; lower is faster or uses less peak RSS.

| Mode | N | Batch | Security | Workers | Seeds | Total ratio | Proof ratio | Witness ratio | Verify ratio | Peak RSS ratio |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| arithmetic | 512 | 1 | 100 | 1 | 8 | 1.016 | 1.018 | 0.846 | 1.001 | 1.007 |
| arithmetic | 512 | 1 | 128 | 1 | 8 | 1.015 | 1.017 | 0.848 | 1.001 | 1.009 |
| arithmetic | 512 | 3 | 100 | 1 | 8 | 1.009 | 1.016 | 0.680 | 0.996 | 1.001 |
| arithmetic | 512 | 3 | 128 | 1 | 8 | 1.007 | 1.013 | 0.677 | 1.000 | 0.998 |
| arithmetic | 1024 | 1 | 100 | 1 | 8 | 1.010 | 1.018 | 0.637 | 1.000 | 1.004 |
| arithmetic | 1024 | 1 | 128 | 1 | 8 | 1.006 | 1.013 | 0.640 | 0.999 | 1.001 |
| arithmetic | 1024 | 3 | 100 | 1 | 8 | 0.993 | 1.015 | 0.423 | 1.001 | 0.999 |
| arithmetic | 1024 | 3 | 128 | 1 | 8 | 0.995 | 1.012 | 0.423 | 0.998 | 0.995 |

11 primary metric comparisons exceed the observed +1.0% review threshold. See confirmation-candidates.json; inspect seed ratios before choosing confirmation cells.
