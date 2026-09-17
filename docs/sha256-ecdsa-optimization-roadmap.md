# SHA-256/P-256 prover optimization roadmap

The current work prioritizes proving speed while preserving public APIs, proof
format, transcript operations, and security settings. Baseline: `8598a1a6`;
the six subsequent ownership refactorings remain intact.

| Area | Result |
| --- | --- |
| Analytic constant groups | Implemented: real groups plus one algebraic padding contribution. |
| Bit-driven GKR kernels | Rejected: both broad and narrowed defaults produced confirmed regressions. Experimental implementations remain in local artifacts only. |
| Workspace and bookkeeping | Rejected: the combined prototype slowed both warm serial proving and first proofs. Removed rather than enabled by default. |
| Compact P-256 coefficients | Implemented: one validated Montgomery tail, direct run emission, borrowed inner adapter, bounded folding into final dense output. |

See [measurements and rejected experiments](gkr-four-optimization-results.md)
for absolute results, uncertainty, correctness coverage, and remaining limits.
The completed finalization campaign is deliberately focused following the
request to finish promptly. The main ten-thread fixture improves from 88.2 to
70.2 ms total proving and from 38.2 to 35.3 ms GKR; proving peak heap remains
220.3 MiB. Small SHA/u32 follow-ups remain inconclusive, and the short screen flags
MultiSwap batch-1 first-proof timing for further validation. It must not be described as an exhaustive no-regression proof.

Possible follow-up work: isolate the workspace package into smaller experiments,
then resume the full held-out 6/12/24-block performance matrix. GPU execution,
deeper forest schedules, setup-matrix removal, and cross-proof scratch retention
remain outside this change.
