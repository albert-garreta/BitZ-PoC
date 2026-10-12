Falcon speed integration qualification

Finite-sample comparison on this machine and these inputs only. Each seed contributes the median of repeated proofs in one process per variant. Reported 95% bootstrap intervals resample paired seed ratios, not individual proofs; three seeds give weak uncertainty estimates and do not establish nonregression. Missing fixture caches are primed in a separate untimed process before either paired measurement. RSS is process lifetime high-water memory, including fixture setup and warmups, not per-proof allocated memory. Preparation and fixture generation are excluded from total_prover_ms.

120 completed pairs across 40 cells. Ratios are candidate / baseline; lower is faster or uses less peak RSS.

| Mode | N | Batch | Security | Workers | Seeds | Total ratio | Proof ratio | Witness ratio | Verify ratio | Peak RSS ratio |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| arithmetic | 512 | 1 | 100 | 16 | 3 | 1.173 | 1.184 | 0.855 | 0.997 | 0.995 |
| arithmetic | 512 | 1 | 128 | 16 | 3 | 0.839 | 0.840 | 0.828 | 1.001 | 1.004 |
| arithmetic | 512 | 3 | 100 | 16 | 3 | 0.999 | 1.004 | 0.867 | 0.991 | 0.984 |
| arithmetic | 512 | 3 | 128 | 16 | 3 | 1.000 | 1.003 | 0.888 | 0.998 | 0.993 |
| arithmetic | 512 | 32 | 100 | 16 | 3 | 0.963 | 0.976 | 0.665 | 0.942 | 0.980 |
| arithmetic | 512 | 32 | 128 | 16 | 3 | 0.969 | 0.977 | 0.753 | 0.952 | 0.990 |
| arithmetic | 512 | 513 | 100 | 16 | 3 | 0.906 | 0.922 | 0.698 | 0.872 | 0.999 |
| arithmetic | 512 | 513 | 128 | 16 | 3 | 0.889 | 0.906 | 0.661 | 0.871 | 0.986 |
| arithmetic | 512 | 1024 | 100 | 16 | 3 | 0.841 | 0.862 | 0.596 | 0.805 | 0.976 |
| arithmetic | 512 | 1024 | 128 | 16 | 3 | 0.895 | 0.918 | 0.590 | 0.807 | 0.993 |
| arithmetic | 1024 | 1 | 100 | 16 | 3 | 0.975 | 0.988 | 0.648 | 0.997 | 1.001 |
| arithmetic | 1024 | 1 | 128 | 16 | 3 | 0.984 | 0.998 | 0.624 | 0.995 | 1.006 |
| arithmetic | 1024 | 3 | 100 | 16 | 3 | 0.977 | 0.986 | 0.660 | 0.987 | 0.999 |
| arithmetic | 1024 | 3 | 128 | 16 | 3 | 0.967 | 0.980 | 0.722 | 0.996 | 0.993 |
| arithmetic | 1024 | 32 | 100 | 16 | 3 | 0.927 | 0.960 | 0.562 | 0.913 | 0.969 |
| arithmetic | 1024 | 32 | 128 | 16 | 3 | 0.941 | 0.968 | 0.562 | 0.927 | 0.969 |
| arithmetic | 1024 | 513 | 100 | 16 | 3 | 0.889 | 0.912 | 0.555 | 0.824 | 1.004 |
| arithmetic | 1024 | 513 | 128 | 16 | 3 | 0.898 | 0.921 | 0.558 | 0.831 | 0.997 |
| arithmetic | 1024 | 1024 | 100 | 16 | 3 | 0.831 | 0.867 | 0.459 | 0.790 | 1.034 |
| arithmetic | 1024 | 1024 | 128 | 16 | 3 | 0.834 | 0.869 | 0.441 | 0.790 | 1.038 |
| full | 512 | 1 | 100 | 16 | 3 | 0.985 | 0.986 | 0.960 | 1.008 | 0.991 |
| full | 512 | 1 | 128 | 16 | 3 | 0.904 | 0.901 | 0.941 | 1.003 | 1.003 |
| full | 512 | 3 | 100 | 16 | 3 | 1.001 | 1.005 | 0.940 | 0.992 | 0.994 |
| full | 512 | 3 | 128 | 16 | 3 | 1.000 | 1.002 | 1.005 | 0.984 | 1.003 |
| full | 512 | 32 | 100 | 16 | 3 | 0.991 | 0.996 | 0.932 | 0.967 | 1.010 |
| full | 512 | 32 | 128 | 16 | 3 | 0.932 | 0.934 | 0.937 | 0.981 | 0.981 |
| full | 512 | 513 | 100 | 16 | 3 | 0.970 | 0.975 | 0.952 | 0.734 | 1.003 |
| full | 512 | 513 | 128 | 16 | 3 | 0.960 | 0.963 | 0.939 | 0.752 | 1.002 |
| full | 512 | 1024 | 100 | 16 | 3 | 0.933 | 0.942 | 0.897 | 0.626 | 1.005 |
| full | 512 | 1024 | 128 | 16 | 3 | 0.943 | 0.952 | 0.908 | 0.643 | 1.006 |
| full | 1024 | 1 | 100 | 16 | 3 | 1.009 | 1.015 | 0.854 | 1.009 | 0.974 |
| full | 1024 | 1 | 128 | 16 | 3 | 0.992 | 0.997 | 0.873 | 0.994 | 0.969 |
| full | 1024 | 3 | 100 | 16 | 3 | 0.977 | 0.982 | 0.921 | 0.996 | 0.999 |
| full | 1024 | 3 | 128 | 16 | 3 | 0.988 | 0.993 | 0.919 | 1.005 | 1.017 |
| full | 1024 | 32 | 100 | 16 | 3 | 0.963 | 0.977 | 0.872 | 0.964 | 1.001 |
| full | 1024 | 32 | 128 | 16 | 3 | 0.973 | 0.986 | 0.897 | 0.956 | 1.008 |
| full | 1024 | 513 | 100 | 16 | 3 | 0.962 | 0.969 | 0.940 | 0.666 | 1.000 |
| full | 1024 | 513 | 128 | 16 | 3 | 0.970 | 0.979 | 0.936 | 0.696 | 1.001 |
| full | 1024 | 1024 | 100 | 16 | 3 | 0.930 | 0.948 | 0.871 | 0.553 | 1.000 |
| full | 1024 | 1024 | 128 | 16 | 3 | 0.934 | 0.947 | 0.872 | 0.583 | 0.999 |

3 primary metric comparisons exceed the observed +1.0% review threshold. See confirmation-candidates.json; inspect seed ratios before choosing confirmation cells.
