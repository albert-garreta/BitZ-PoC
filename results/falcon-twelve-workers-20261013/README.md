# Qualify optimized arithmetic Falcon binding at 12 workers

The 12-worker production path now uses the existing optimized binding constructor and evaluator above the small-batch cutoff. The previous selector enabled these paths at 1, 8 and 16 workers but skipped 12. This caused a large, avoidable proving and verification cliff. The change is one runtime selector line in `src/piop/spartan/falcon1024_algebraic/arithmetic.rs`; added tests exercise the non-power-of-two worker pool and both sides of its cutoff.

The cutoff remains batch >16 for Falcon-512 and >8 for Falcon-1024. Other worker policies, including Falcon-512's batch-1024 special case at 2/4 workers, are unchanged. Field arithmetic, coefficient validation, transcript challenges/domains, security/grinding parameters and proof format are unchanged. This campaign uses canonical algebraic Falcon v9 at **batch 1024, 128-bit security** unless explicitly labelled correctness coverage.

## Production result

Times are median total commit + prove milliseconds, excluding verification. Each row has 21 cold and 21 repeated samples per configuration across seeds 42/43/44. Both binaries use `falcon-hybrid` without `bench-internals`, no experiment overrides, and the same pinned 12 physical cores. The baseline is commit `bfd6eaf98901bec2bac4504f649d48028cc87f2c`.

| Degree | Mode | Prover: baseline → new, ms | Prover reduction | Verification: baseline → new, ms |
|---|---|---|---|---|
| 512 | cold | 45.603 → 34.048 | 25.34% | 21.836 → 8.003 |
| 512 | repeated | 42.448 → 31.820 | 25.04% | 22.102 → 9.077 |
| 1024 | cold | 83.936 → 59.281 | 29.37% | 41.440 → 14.704 |
| 1024 | repeated | 78.493 → 56.407 | 28.14% | 42.439 → 15.977 |

All three per-seed medians improve for both degrees and modes. The earlier same-binary experiment independently isolated the binding selector: Falcon-512 repeated time fell 42.388 → 31.751 ms; Falcon-1024 fell 77.892 → 56.408 ms. That experiment used the previously frozen `candidate-v2` with **explicit PCS four-tile selection for both strategies**, because it predates the PCS default promotion. Its control differed only in `BITZ_ARITHMETIC_BINDING_STRATEGY=legacy|optimized`. `causal-review.txt` records all cold/repeated and per-seed comparisons. The production table above is the independent confirmation with the new default.

## Worker scaling and regressions

New-production repeated medians, using physical cores in the established order. The 12-worker row is from the production phase; 1/8/16 rows are from the subsequent regression phase with the same frozen binaries and methodology:

| Workers | Falcon-512, ms | Falcon-1024, ms |
|---|---|---|
| 1 | 139.059 | 268.972 |
| 8 | 28.459 | 51.349 |
| 12 | 31.820 | 56.407 |
| 16 | 28.665 | 49.007 |

Twelve physical cores still trail 8/16 in these workloads after the binding fix. This is not a monotonic slowdown above 12. The remaining forest costs are discussed below. The 22 ms Falcon-512 target remains unmet.

The unchanged 1/8/16 policies received 21 repeated samples per binary/configuration. Small positive changes in the first comparison motivated an independent 8/16 confirmation, also with 21 repeated samples per binary/configuration. Negative changes mean faster:

| Run | Degree | Workers | Baseline → new, ms | Change |
|---|---|---|---|---|
| regression | 512 | 1 | 138.896 → 139.059 | +0.12% |
| regression | 512 | 8 | 28.330 → 28.459 | +0.45% |
| regression | 512 | 16 | 28.468 → 28.665 | +0.69% |
| regression | 1024 | 1 | 269.794 → 268.972 | -0.30% |
| regression | 1024 | 8 | 51.191 → 51.349 | +0.31% |
| regression | 1024 | 16 | 49.264 → 49.007 | -0.52% |
| regression-confirmation | 512 | 8 | 28.319 → 28.328 | +0.03% |
| regression-confirmation | 512 | 16 | 28.178 → 28.362 | +0.65% |
| regression-confirmation | 1024 | 8 | 51.154 → 51.042 | -0.22% |
| regression-confirmation | 1024 | 16 | 48.454 → 48.642 | +0.39% |

Pooled regression-median changes stay between -0.52% and +0.69%, but seed-level variation is larger. In the confirmation, Falcon-512 at 16 workers changes by +5.22% for seed 42 and -0.92%/-0.31% for seeds 43/44; Falcon-1024 at 16 workers has a +1.63% seed. These mixed results must not be described as proof of zero performance regression on every workload. The code change selects a different path only at 12 workers; binary layout and run-to-run timing can still affect other measurements. No correctness regression was observed.

## Why 12 physical cores still have a penalty

Separate traced diagnostics use three seeds, one warmup and three measured proofs per seed (nine samples per row). They are not included in untraced latency qualification. Construction is included within whole binding; nested wall spans must not be added.

| Degree | Stage at 12 physical workers | Baseline → new, ms |
|---|---|---|
| 512 | Binding construction | 10.864 → 0.545 |
| 512 | Whole binding | 15.124 → 4.749 |
| 512 | Forest | 14.482 → 14.494 |
| 1024 | Binding construction | 21.649 → 1.074 |
| 1024 | Whole binding | 29.034 → 7.974 |
| 1024 | Forest | 27.435 → 27.215 |

The host is a Ryzen 9 9950X3D. Physical cores 0–7 share 96 MiB L3; 8–15 share 32 MiB. The established 8-worker placement fits the larger cache, while 12 physical workers span both. Forest stable row ownership is also enabled at 16 workers but intentionally excludes 12: power-of-two row counts cannot remain divisible by 12 across high-bit folds. Those restrictions remain intact.

| Degree | Forest, 8 physical | Forest, 12 physical | Forest, 16 physical | Forest, 12 logical in first cache |
|---|---|---|---|---|
| 512 | 10.847 | 14.494 | 11.651 | 11.120 |
| 1024 | 19.851 | 27.215 | 21.193 | 20.238 |

This supports a cache-locality/scheduling explanation for the remaining forest penalty. It is not a hardware-counter measurement of cache traffic. `scheduling-analysis.txt` explains the evidence and a possible future row-grain experiment. No forest scheduler change is promoted here.

## Separate placement experiment

Keeping all 12 logical workers on the larger-cache group uses **8 physical cores plus 4 SMT siblings**. It is not a 12-physical-core result or an additional source-code optimization. Production-binary measurements confirm:

| Degree | Mode | 12 physical prover, ms | 12 logical cache-local prover, ms | Verification: physical → cache-local, ms |
|---|---|---|---|---|
| 512 | cold | 34.048 | 31.833 | 8.003 → 8.049 |
| 512 | repeated | 31.820 | 28.814 | 9.077 → 9.110 |
| 1024 | cold | 59.281 | 57.181 | 14.704 → 15.256 |
| 1024 | repeated | 56.407 | 51.878 | 15.977 → 16.408 |

The cache-local placement improves every prover per-seed median, but Falcon-1024 verification gets slightly slower. It remains an explicitly selected benchmark configuration; no process-wide affinity policy is installed. Worker CPUs are `[6,7,4,5,2,3,0,1,22,23,20,21]`, compared with physical-12 `[6,7,4,5,2,3,0,1,14,15,12,13]`. Main and auxiliary-pool CPUs remain 6.

## Memory and proofs

Process peak and retained RSS are distinct metrics. The peak table reports the median of each process's maximum observed high-water mark, not a pooled median of per-trial high-water readings. Retained RSS is sampled **after verification and dropping the proof**, while Prepared and the input corpus remain live. It includes allocator-retained pages; it is not a persistent proof/scratch allocation measure.

| Degree | Mode | Peak RSS: baseline → new, MiB | Retained RSS: baseline → new, MiB |
|---|---|---|---|
| 512 | cold | 87.98 → 80.18 | 43.01 → 72.92 |
| 512 | repeated | 95.97 → 85.84 | 43.87 → 44.00 |
| 1024 | cold | 166.68 → 137.59 | 76.86 → 77.03 |
| 1024 | repeated | 184.86 → 172.93 | 78.66 → 78.35 |

Peak RSS falls, but Falcon-512 cold retained RSS rises by about 29.9 MiB. Repeated retained RSS is effectively unchanged. Do not infer that every memory metric improves. Cached-vector counters are unavailable in production builds and recorded as null rather than zero.

For every matching input, complete proof Debug digests, source roots and payload sizes remain identical. Debug-digest comparison checks complete in-build proof contents, not a canonical wire codec. Across seeds 42/43/44, Falcon-512 payloads are 256,368 / 256,112 / 256,848 bytes; Falcon-1024 payloads are 321,272 / 321,304 / 321,432 bytes. No proof-size or security change is introduced.

## Validation and method

- Native Falcon suite: **361 passed, zero failed, three ignored**. The expanded ordinary tests cover complete proofs at batches 17/1024, security 100/128, Bits/Words binding and worker pools 1/2/3/4/8/12/16; cross-pool verification; owned/borrowed arithmetic proofs and transcripts at/below/above each cutoff; canonical-field rejection; constructor/evaluator equivalence for 61-/127-bit fields and Boolean/non-Boolean challenges. Ignored tests were not rerun in this narrow selector campaign.
- Eleven synthetic runner tests passed. The new runner preserves the prior frozen runner/helper files and adds strict per-configuration affinity validation.
- The completed benchmark audit verifies **1,396 proofs**, including warmups, across all recorded phases. Correctness-only fresh-process cases include batches 3/129 at security 100/128 for both degrees at 12 workers, against the previous production binary. Every proof verifies and matches its baseline input, source, Debug digest and payload across threads, placements and phases.
- Benchmark variants run serially in randomized matched blocks. No builds/tests overlap timed benchmarks. Cold means the first proof in a fresh process/Prepared, seven processes per seed; machine caches are not flushed. Repeated means two warmups and seven samples per seed sharing Prepared. Within-process samples are correlated. Preparation, input generation, resource sampling and verification are outside prover timing.
- Worker order for physical-core runs is `[6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9]`. Affinity is read back for every trial. `host.json` records cache topology, governor, SMT and page size. This host uses 4 KiB pages; ignore the legacy forest trace's hard-coded “MB faulted” display. Trace worker-nanosecond totals are summed CPU work, not elapsed wall time.

The production executable SHA-256 is `bf303cb59a2e6830f0beb97670bd693aaef0d22fd5ad54bb7304696e0c850511`; the frozen previous-production SHA-256 is `b4eb01b1fa8f40b0c0b0c61d82372fe94070106b2d5023a2b47f7e1488150415`. `candidate-production-source.json`, its patch, Cargo build records and `production-build-audit.json` tie the executable to the final source and verify that `bench-internals` is absent. `source-review.txt`, `runner-review.txt`, `topology-runner-review.txt`, `causal-review.txt` and `production-review.txt` provide independent inspections.

`summary.json` and `profile-summary.json` contain all pooled and per-seed metrics. `forest-summary.json` contains parsed internal forest timers. Phase manifests freeze binaries, runner/helpers, commands, flags, inputs and affinity; each raw result carries checked command/stdout/stderr/process hashes. Revalidate while the frozen binaries remain available:

```sh
python3 scripts/falcon_worker_optimization.py --output results/falcon-twelve-workers-20261013 --summarize-only
```

`run-qualification.py`, `build-candidate.py`, `run-tests.py` and the phase command records document reproduction. The final regression-confirmation phase is independently frozen in its manifest. The previous campaign's runner and evidence remain unchanged.
