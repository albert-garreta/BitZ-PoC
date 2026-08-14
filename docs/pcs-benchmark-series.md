# F2Z PCS benchmark commands

The timing-series command captures witness sizes `2^20` through `2^30` and
writes the headline and diagnostic-breakdown CSV/LaTeX tables. The requested
Ligerito profile is its only configuration input:

```sh
python3 scripts/bench_pcs_series.py \
  --profile custom:3:4 \
  --out-dir bench_results/pcs-custom-3-4-RUN
```

This command does **not** capture, read, or export intervals, and it does not
write HTML. Its report directory contains only:

- `headline.csv` and `headline.tex`
- `prover-breakdown.csv` and `prover-breakdown.tex`

Capture mode requires a fresh output directory. To rebuild only these four
tables from an existing bundle without running Rust:

```sh
python3 scripts/bench_pcs_series.py \
  --profile custom:3:4 \
  --out-dir bench_results/pcs-custom-3-4-RUN \
  --report-only
```

## Measurement boundaries

The clean headline pass uses one excluded warmup followed by 21 proving and
verification repetitions. Peak heap is the tracked Rust-heap high-water mark
from one additional prove with the commitment hint still live; it is neither
RSS nor a median. The phase breakdown is one separately instrumented diagnostic
prove and must not be subtracted from the clean headline median.

Requested profile and Rust-resolved geometry are recorded separately and
reconciled between the clean and diagnostic passes. Named embedded profiles
(`slim`, `fast`, and `secure`) resolve to the benchmark's unaudited rate-1/4
small-instance configuration at exponents 20 and 21. `slim3` and `r8` are
rate-1/8 ad-hoc probes throughout their sweeps. Explicit `custom:*` profiles
are derived and validated for the full 20--30 sweep.

## Optional interval capture

Intervals are a distinct, instrumented measurement. Run them only when needed:

```sh
python3 scripts/bench_pcs_intervals.py \
  --profile custom:3:4 \
  --out-dir bench_results/pcs-custom-3-4-RUN/raw/intervals
```

This records one warmup plus 21 measured canonical JSONL traces per exponent.
Each exponent is validated before its directory is promoted. It does not
aggregate the traces or build HTML.

## Optional interval dashboard

After both timing and interval capture, explicitly build the two dashboard
inputs and then the standalone HTML. These commands do not run Rust:

```sh
python3 scripts/export_pcs_tables.py \
  --headline bench_results/pcs-custom-3-4-RUN/raw/headline.csv \
  --breakdown bench_results/pcs-custom-3-4-RUN/raw/breakdown.csv \
  --out-dir bench_results/pcs-custom-3-4-RUN/report \
  --expected-profile custom:3:4 \
  --with-dashboard-data \
  --force

python3 scripts/export_f2z_intervals.py \
  --logs-dir bench_results/pcs-custom-3-4-RUN/raw/intervals \
  --output bench_results/pcs-custom-3-4-RUN/report/interval-data.json \
  --expected-profile custom:3:4 \
  --force

python3 scripts/build_f2z_dashboard.py \
  --input bench_results/pcs-custom-3-4-RUN/report/dashboard-data.json \
  --interval-input bench_results/pcs-custom-3-4-RUN/report/interval-data.json \
  --output f2z-pcs-fresh-comparison.html
```

The dashboard uses the measured interval trial nearest median total time for
real bar geometry and Type-7 P10--P90 deciles across the 21 measured trials.
