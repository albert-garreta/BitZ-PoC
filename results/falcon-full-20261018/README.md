# Full Falcon-512 at 1024 signatures (hybrid prover)

Raw data for the full-Falcon part of the Falcon note: John's `falcon_hybrid`, which proves SHAKE256 HashToPoint, the ring and the norm, at 128 bits. Every run is 1024 Falcon-512 signatures on an Apple M5 (10 cores, AC power), serialized by `scripts/bench_gate.py`. `build.zsh` builds the bench with `--profile release --features falcon-hybrid,bench-internals` and `-C target-cpu=native`:

- `hybrid-head`: built from `b9a30ee66`, used for `baseline/`.
- `hybrid-ports`: built from `08dee73eb`, used for `ladder/` and `rows/`. That commit adds the bridge rows and the small prime to the hybrid.

`binaries.txt` in each folder holds the md5 of its binary.

## Switches

| Change | Variable |
|---|---|
| Optimized binder replay (on by default only on Linux x86-64) | `BITZ_BENCH_BINDER_REPLAY=on` |
| Per-site grinding | `BITZ_FALCON_GRINDING_MODEL=per-site` |
| Bridge row variables `t` | `BITZ_FALCON_BRIDGE_ROWS=13..16` |
| Small prime: `[2^(127-t), 2^(128-t) - 2^(116-t)]`, unsplit bridge | `BITZ_FALCON_PRIME=small` |

## Results

The tables below are pooled medians from `pooled.py`, over all measured proves of the clean runs. Prove includes the witness commitment.

10 threads:

| Configuration | Prove | Verify | Proof | Peak RSS |
|---|---:|---:|---:|---:|
| As built on macOS (union grinding, reference binder) | 542 ms | 28.0 ms | 697,418 B | 2.43 GiB |
| + optimized binder | 511 ms | 27.9 ms | 697,418 B | 2.43 GiB |
| + per-site grinding | 405 ms | 28.1 ms | 698,106 B | 2.43 GiB |
| + 2^16 bridge rows | 411 ms | 27.9 ms | 556,498 B | 2.44 GiB |
| + small prime (2^16 rows) | 3,089 ms | 27.9 ms | 549,970 B | 2.47 GiB |
| Per-site, small prime, 2^15 rows | 595 ms | 27.6 ms | 568,506 B | 2.40 GiB |
| Per-site, small prime, 2^14 rows | 600 ms | 27.8 ms | 600,186 B | 2.41 GiB |
| Per-site, small prime, 2^13 rows | 383 ms | 27.4 ms | 664,754 B | 2.43 GiB |

One thread, prove / verify:
- As built: 2,346 ms / 38.9 ms.
- Per-site: 1,664 ms / 39.6 ms.
- Per-site with 2^16 rows: 1,718 ms / 39.4 ms.
- Small prime at 2^16 rows: 16,508 ms / 38.9 ms.
- Small prime at 2^13 rows: 1,612 ms / 38.3 ms.

Per-site difficulties of the prime-field sites, from the `*-report.err` files:

| Prime | norm instance | relation batching | outer point | integer rounds | linear | binder | projection |
|---|---:|---:|---:|---:|---:|---:|---:|
| default, `p >= 2^125` | 7 | 3 | 8 | 5 | 14 | 4 | 8 |
| small at t = 13, `p >= 2^114` | 18 | 14 | 19 | 16 | 25 | 15 | 19 |
| small at t = 16, `p >= 2^111` | 21 | 17 | 22 | 19 | 28 | 18 | 22 |

The `linear` numerator in `FalconSecurityNumerators::linear` includes the batch size (about 2^11 at 1024). Together with the integer rounds, this is what makes the small prime expensive past 2^13 rows.

## Campaigns

- `baseline/` (`baseline.zsh`): union or per-site grinding, each with and without the binder replay, at `b9a30ee66`.
  - `PB-trace.err`: span stage timings of per-site with the binder. Read them with `stages.py` and `verify_stages.py`; the spans add overhead.
  - `PB-report.err`: the per-site grinding report.
- `ladder/` (`ladder.zsh`):
  - Steps S0 to S5 in two interleaved repetitions of medians of 5.
  - One thread for S0, S2, S3 and S4.
  - The grinding reports of S3 and S4.
- `rows/` (`rows.zsh`):
  - Small prime at `t = 13..16` and the default prime at 13 and 16 rows, two repetitions each, plus their reports.
  - The small prime at 2^13 rows on one thread.
- Excluded runs: `ladder/S3-t10-r2`, `ladder/S4-t10-r2` and `rows/default-t16-r1`. They overlapped another session's unlocked work. In them the native Falcon check, which none of these changes touch, took 23 to 26 ms instead of 18 ms, the all-core clock fell to 3.0 to 3.3 GHz, or prove took 364 ms against 327 to 329 ms in the other runs.
- `test-2-*.log`: the hybrid library tests, which compile for the Falcon-1024 K=11 instantiation (the code is generic in the degree).
  - Default switches: 102 passed.
  - Per-site: 99/102.
  - `BITZ_FALCON_BRIDGE_ROWS=16`: 99/102.
  - Small prime, per-site and 16 rows: 96/102.

  The failing tests pin the default prime families, two-limb column sums at 128 bits, or the union-bound accounting. The full proof round trips and mutation rejections pass under every setting. `test-1-*.log` is the first round, before the 100-bit family followed `t` and before the per-site projection site was clamped to one bit.
- Scripts:
  - `summarize.py`: per-run medians, with `--breakdown` for proof sizes.
  - `pooled.py`: the tables above.
  - `build.zsh`, `build-and-ladder.zsh` and `test.zsh`, plus the campaign scripts above.

  The scripts hold absolute paths to `bench_results/full-falcon-20261018` in the `falcon-flamegraph` worktree.
