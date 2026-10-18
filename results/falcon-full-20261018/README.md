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

## Seed-averaged results (use these)

With fixed inputs, every trial of a run replays the same transcript, so each grinding site is a single geometric draw (mean `2^g` hashes). Many small sites average out, but a single large site does not.

The bench reports the work actually spent per category (`grinding_diagnostics`, read with `nonces.py`). In the seed-42 runs, the 25-bit linear site of the small prime at 2^13 rows found its nonce after about 1M hashes, where 33.5M are expected. The 28-bit site at 2^16 rows took 896M hashes, against about 270M expected.

`seeds/` (`seeds.zsh`) therefore repeats every configuration for input seeds 1 to 5. Each run is a warm-up plus three proves on 10 threads; `seed_means.py` averages each seed's median, including the seed-42 runs below. `seeds/S4-s1` and `seeds/S5-s1` are excluded as disturbed: the native check took 24 to 29 ms instead of 18, commit took 239 and 1,236 ms instead of about 80, and the clock was 3.2 to 3.3 GHz.

| Configuration, 10 threads | Seeds | Prove (mean) | Range | Verify | Proof | Grinding work |
|---|---:|---:|---:|---:|---:|---:|
| As built on macOS (union, reference binder) | 6 | 552 ms | 524-601 | 28.3 ms | 698 kB | 32.3 M hashes |
| + optimized binder | 6 | 516 ms | 497-540 | 28.2 ms | 698 kB | 32.3 M |
| + per-site grinding | 6 | 408 ms | 382-437 | 28.1 ms | 698 kB | 4.6 M |
| + 2^16 bridge rows | 6 | 422 ms | 396-461 | 28.2 ms | 557 kB | 5.1 M |
| + small prime (2^16 rows) | 5 | 1,651 ms | 591-3,257 | 28.1 ms | 550 kB | 443 M |
| Per-site, small prime, 2^13 rows | 5 | 434 ms | 383-519 | 27.4 ms | 665 kB | 30.1 M |
| Per-site, tight linear numerator | 6 | 411 ms | 387-484 | 28.8 ms | 697 kB | 5.2 M |
| Tight: small prime, 2^16 rows | 6 | 468 ms | 446-495 | 28.4 ms | 551 kB | 38.6 M |
| Tight: small prime, 2^13 rows | 6 | 379 ms | 373-393 | 27.5 ms | 664 kB | 10.2 M |

"Tight" is the bench-only accounting experiment `BITZ_FALCON_LINEAR_NUMERATOR=tight`. It counts the linear block's numerator as `m + d + 6` instead of `m + d + B + 5`.

The block draws one multilinear point over the linear rows and signatures (equality weights of total degree `m + d`), then `eta` (used up to its fifth power), then one merge coefficient. No prover message intervenes, so its total degree is at most `m + d + 6`. We found no use of the batch size `B` in that block.

With John's numerator the small prime does not pay at 1024 signatures. With the tight one it pays at 2^13 rows: 379 ms against 408 to 411 ms, with little spread.

## Results with seed 42 only

The tables below are pooled medians from `pooled.py`, over all measured proves of the clean seed-42 runs. Prove includes the witness commitment. The small-prime rows depend on the grinding luck described above.

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
- `tight/` (`tight.zsh`, binary `hybrid-tight`): built from `08dee73eb` plus the opt-in `BITZ_FALCON_LINEAR_NUMERATOR` switch (`build-tight.rev`). Contains the tight-numerator runs at seed 42 and their grinding reports, including the union model.
- `seeds/` (`seeds.zsh`): seeds 1 to 5 for every configuration, with the same binary.
- Scripts:
  - `summarize.py`: per-run medians, with `--breakdown` for proof sizes.
  - `pooled.py`: the seed-42 tables.
  - `seed_means.py`: the seed-averaged table.
  - `nonces.py`: grinding work actually spent per category.
  - `build.zsh`, `build-and-ladder.zsh` and `test.zsh`, plus the campaign scripts above.

  The scripts hold absolute paths to `bench_results/full-falcon-20261018` in the `falcon-flamegraph` worktree.
