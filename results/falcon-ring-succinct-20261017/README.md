# Keyed ring check and bridge rows (Falcon-512, 32K signatures)

Raw data for the configuration table in the "Keyed integer ring check and bridge rows" section of `docs/falcon-aerie-experiments-20261017.md`. The binary is built from `falcon-ring-succinct` `d0ed8cfc1`. Runs are Falcon-512 at 128 bits on an Apple M5, gated by `scripts/bench_gate.py`, with per-site grinding and the optimized binder, on 10 threads unless noted.

`campaign.zsh` runs configurations A–I over the prime family (`BITZ_FALCON_PRIME`), the ring check (`BITZ_ARITHMETIC_RING`) and the bridge row variables (`BITZ_FALCON_BRIDGE_ROWS`):

| Run | Prime | Ring | Bridge rows |
|---|---|---|---|
| A | default | canonical | 13 |
| B | default | keyed | 13 |
| C | small | canonical | 13 |
| D | small | keyed | 13 |
| E | small | keyed | 14 |
| F | small | keyed | 15 |
| G | small | keyed | 16 |
| H | default | keyed | 16 |
| I | default | canonical | 16 |

`1k` suffixes are batch-1024 runs; `1t` suffixes are single-thread runs. `trace-D.*` is a span trace of D used for the verifier breakdown.

- `runs/`: bench JSON (`*.jsonl`) with `/usr/bin/time -l` and the trace in `*.stderr`; `clock.log` records clock speed and power for each run; `binaries.txt` holds the binary's hash and commit.
- `summarize.py`: prints the table.
- `sanity/`: small correctness runs.
- `test-*.log`: the algebraic test suite on this branch. `test-1.log` is the first run, which caught the keyed ring's Montgomery-twiddle bug (2 failures); `test-2.log` is after the fix (98 passed).

These are copies of `bench_results/ring-20261017` from the `falcon-ring-succinct` worktree. The 79 MB input-case cache is omitted; the bench regenerates it deterministically from seed 42.
