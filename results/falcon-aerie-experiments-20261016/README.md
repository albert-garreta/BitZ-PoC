# Per-site against union-bound grinding (Falcon, Aerie comparison)

Raw data for the grinding A/B in `docs/falcon-aerie-experiments-20261017.md`. Both arms use one binary: John's `falcon-poc` `e140a3f3` with the bench-only switches of commit `17482040f`. They differ only in `BITZ_FALCON_GRINDING_MODEL=union|per-site`. Runs are on an Apple M5, gated by `scripts/bench_gate.py`, with seed-42 inputs.

- `runs-grind/` (from `grind-ab.zsh`): algebraic Falcon at 128 bits.
  - Falcon-512 at batch 1 and 1024, on 1 and 10 threads, interleaved repeats `-r1`/`-r2`, plus Falcon-1024 at batch 1024.
  - `report-*` files are the per-site grinding reports (`BITZ_FALCON_PRINT_GRINDING=1`).
  - `trace-*` files are span traces used for the verifier attribution.
- `runs-full/` (from `full-ab.zsh`): full Falcon-512 (SHAKE, HashToPoint and ring) at batch 1 and 1024, on 1 and 10 threads.
- `*.time` files are `/usr/bin/time -l` outputs; `clock.log` records the clock speed (cycles/user) and power state of each run.
- `summarize.py` prints the medians.
- `test-default.log` is the algebraic test suite (`--lib falcon1024_algebraic`) on the experiment build: 92 passed.

These are copies of `bench_results/aerie-20261016` and `aerie-20261017` from the `falcon-aerie-experiments` worktree; the build logs other than the first are omitted.
