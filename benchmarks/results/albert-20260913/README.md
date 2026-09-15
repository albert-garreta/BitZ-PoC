# Albert benchmark investigation — 2026-09-13 snapshot

This directory preserves the benchmark evidence independently of the live `PerfRuns` directories. It contains the code/configuration audit reports, campaign commands and scripts, measured samples, memory records, logs, summary tables, interval visualizations, and the Binius parameter workbook. `snapshot-manifest.json` records the captured files and their hashes. Paths below are relative to this directory.

- [Original completed campaign](PerfRuns/albert-requested-_f62mead/REPORT.md)
- [Draft response to Albert](PerfRuns/albert-requested-_f62mead/ALBERT-REPLY.md)
- [Partial sequential rerun report](PerfRuns/albert-sequential-9h7_or3i/REPORT.md)
- [Sequential commands](PerfRuns/albert-sequential-9h7_or3i/COMMANDS.md)
- [Parameter workbook](outputs/01a09ba3-aa1c-7f31-8040-6f830edab259/binius-focused-pcs-parameters.xlsx)

## Interrupted rerun and timing limitations

The sequential rerun is incomplete. Its controller status still reports a running Plonky3 rate-1/4 job, and its generated report predates some samples in the captured raw records. Do not interpret either file as a completed rerun. Use the raw samples to identify fully sampled cases; exclude incomplete cases from five-repetition comparisons.

The user reported stopping the runs, but the Plonky3 log continued appending samples afterward. Compiler validation was subsequently run in this shared checkout, overlapping that residual benchmark activity. Late overlapping timings are not clean benchmark measurements. The original measurements also have the shared-machine contention caveats documented in their reports.

Reports retain their original local absolute links and timestamps. Raw and derived trace streams, temporary process-control metadata, Python caches, and duplicate worker source trees are excluded from this snapshot. They remain in the original local run directories. Recorded source patches, hashes, configuration manifests, per-trial results, and rendered interval summaries are retained. Campaign scripts retain their original output paths; review those paths before replaying them.

## Code validation

See [validation/README.md](validation/README.md) for checks and regression tests of code commit `94c8d26`. These checks are separate from the recorded benchmark campaigns.
