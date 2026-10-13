# Falcon forest, PCS and binding experiments — 2026-10-13

The completed experiments support reducing the PCS initial-message task size from 16 tiles to four for the measured geometries. Wide forest buckets, retaining forest level 4, and binding-construction fusion remain disabled by default. The production build independently confirms a modest total-prover improvement, with unchanged proof digests and payloads.

The selected production policy uses four tiles only for **32 or 64 tiles with 8, 12 or 16 Rayon workers**. Every other geometry and worker count retains 16 tiles. Benchmark overrides remain available behind `bench-internals`. There is no protocol, security, proof-format or verification change: this campaign uses canonical algebraic Falcon v9.

## Production confirmation

The final production build uses `falcon-hybrid` without `bench-internals` and no experiment overrides. All production rows use batch 1024 and 128-bit security. Each row has 21 cold and 21 repeated samples for each binary, across three seeds. Times are median total commit + prove milliseconds.

| Degree | Workers | Cold baseline → production | Repeated baseline → production | Repeated change |
|---|---:|---:|---:|---:|
| 512 | 1 | 144.748 → 144.585 | 138.742 → 138.673 | -0.05% |
| 512 | 8 | 31.689 → 31.325 | 30.038 → 29.319 | -2.39% |
| 512 | 12 | 46.897 → 46.432 | 44.538 → 44.317 | -0.50% |
| 512 | 16 | 31.560 → 30.862 | 29.230 → 28.589 | -2.19% |
| 1024 | 1 | 277.560 → 276.824 | 269.501 → 269.701 | +0.07% |
| 1024 | 8 | 56.529 → 56.317 | 51.432 → 50.981 | -0.88% |
| 1024 | 12 | 84.575 → 84.133 | 78.359 → 77.977 | -0.49% |
| 1024 | 16 | 53.165 → 52.336 | 49.797 → 48.497 | -2.61% |

The Falcon-512 gains are consistent across the three per-seed medians at 8/12/16 workers. Falcon-1024 has smaller, more variable gains at 8/12: one seed is essentially neutral in each repeated case, and a cold seed is slightly slower. Its 16-worker improvement is consistent. Serial results are effectively neutral; Falcon-1024 repeated time is 0.07% higher in the pooled median. These results do not establish zero performance regressions on every workload, and **Falcon-512 has not reached the 22 ms target**. The existing 12-worker legacy binding path remains a major bottleneck.

All matching cases have identical complete proof digests (computed from the Debug representation) and payload sizes. Across the three seeds, batch-1024/security-128 payloads range from **256,112–256,848 bytes for Falcon-512** (median 256,368) and **321,272–321,432 bytes for Falcon-1024** (median 321,304). Every matching seed has the same payload as the baseline. Repeated process peak RSS differs by at most 0.6 MiB in the paired medians. Verifier code and security parameters are unchanged; verification timings and RSS for every configuration are in `summary.json`.

| Degree | Workers | Repeated verification baseline → production, ms | Repeated peak RSS baseline → production, MiB |
|---|---:|---:|---:|
| 512 | 1 | 30.373 → 30.323 | 81.3 → 81.3 |
| 512 | 8 | 9.719 → 9.738 | 83.5 → 83.4 |
| 512 | 12 | 22.864 → 22.479 | 96.4 → 96.3 |
| 512 | 16 | 9.568 → 9.586 | 87.6 → 87.8 |
| 1024 | 1 | 59.547 → 59.364 | 201.8 → 201.9 |
| 1024 | 8 | 16.333 → 16.248 | 171.8 → 171.3 |
| 1024 | 12 | 42.863 → 42.917 | 184.4 → 185.0 |
| 1024 | 16 | 15.276 → 15.105 | 174.9 → 175.0 |

Production executable SHA-256: `b4eb01b1fa8f40b0c0b0c61d82372fe94070106b2d5023a2b47f7e1488150415`. `production-build-audit.json` confirms the post-build source hashes and absence of `bench-internals`. The production campaign itself verified 768 proofs; every paired proof matched the previous production baseline.

## Untraced experiments

Times are median **commit + prove milliseconds**, excluding verification. Negative percentages mean faster than the candidate binary's matched `control` configuration, which explicitly selects the previous defaults. All tables use **batch 1024, security 128**. These are experimental-selector measurements, distinct from the production confirmation above.

The Falcon-512 screen used 21 cold samples and 21 repeated samples per configuration and worker count:

| Configuration | 8 workers, cold | 8 workers, repeated | 12 workers, cold | 12 workers, repeated |
|---|---:|---:|---:|---:|
| Control | 31.134 | 28.738 | 46.423 | 43.121 |
| Frozen production baseline | 31.120 | 28.988 | 46.475 | 43.119 |
| PCS four tiles | 30.459 (−2.17%) | 28.317 (−1.46%) | 45.813 (−1.31%) | 42.302 (−1.90%) |
| PCS two tiles | 30.322 (−2.61%) | 28.107 (−2.20%) | 45.685 (−1.59%) | 42.740 (−0.88%) |
| Wide forest buckets | 31.356 (+0.71%) | 29.050 (+1.09%) | 46.546 (+0.26%) | 43.268 (+0.34%) |
| Retained forest L4 | 33.388 (+7.24%) | 31.183 (+8.51%) | 49.031 (+5.62%) | 46.137 (+6.99%) |
| Binding-construction fusion | 31.021 (−0.36%) | 28.849 (+0.39%) | 46.604 (+0.39%) | 43.074 (−0.11%) |

Four tiles improved every per-seed median in all four screen cases. Two tiles had a 12-worker repeated outlier: seed 43 was 7.13% slower than control despite the lower pooled median. Wide buckets had no consistent end-to-end benefit; retention was slower for every seed. Fusion at eight workers was inconclusive. **At 12 workers the default legacy binder makes the fusion selector inactive**, so that row does not measure the fused constructor.

Independent PCS qualification used 21 repeated samples per configuration, extending coverage to both degrees and 1/8/12/16 workers:

| Degree | Workers | Control, ms | Four tiles, ms (change) | Two tiles, ms (change) |
|---|---:|---:|---:|---:|
| 512 | 1 | 139.098 | 139.397 (+0.21%) | 139.261 (+0.12%) |
| 512 | 8 | 28.948 | 28.109 (−2.90%) | 28.204 (−2.57%) |
| 512 | 12 | 43.186 | 42.764 (−0.98%) | 42.659 (−1.22%) |
| 512 | 16 | 28.826 | 28.208 (−2.15%) | 28.137 (−2.39%) |
| 1024 | 1 | 270.324 | 270.307 (−0.01%) | 270.462 (+0.05%) |
| 1024 | 8 | 51.633 | 51.309 (−0.63%) | 51.134 (−0.97%) |
| 1024 | 12 | 78.934 | 78.265 (−0.85%) | 78.161 (−0.98%) |
| 1024 | 16 | 49.620 | 49.125 (−1.00%) | 48.873 (−1.51%) |

Four tiles improved every per-seed median at 8/12/16 workers for both degrees. One-worker results were neutral or mixed, so its policy stays unchanged. Two tiles sometimes had the lower pooled median; qualification does not establish that four is universally optimal. Four provides the consistent qualified improvement with less task subdivision. In the Falcon-512 geometry, 32 tiles become eight tasks instead of two; Falcon-1024 has 64 tiles. See `summary.json` for verification, memory and per-seed medians, including the separately measured frozen baseline.

## Diagnostic profiles and memory

Profiles are separate traced runs: three seeds, one warmup and three measured proofs per seed, giving nine measured samples per row. They explain mechanisms and are not latency qualification. Forest times below are milliseconds from the completed candidate `profiles` phase.

| Diagnostic | Control, 8 workers | Wide, 8 | Retain, 8 | Control, 12 | Wide, 12 | Retain, 12 |
|---|---:|---:|---:|---:|---:|---:|
| Whole forest | 10.864 | 11.181 | 12.856 | 14.309 | 14.430 | 16.897 |
| Sum of JIT round timers | 1.090 | 1.364 | 1.134 | 0.876 | 1.028 | 0.906 |
| Upper construction | 1.2 | 1.1 | 2.1 | 1.3 | 1.2 | 2.4 |
| L4 rebuild | 0.278 | 0.277 | 0 | 0.528 | 0.539 | 0 |
| L4 retained-arena replacement timer | 0 | 0 | 0.765 | 0 | 0 | 0.902 |

Wide buckets increase each bucket set from 12 KiB to 24 KiB; the measured JIT round cost increased. Retention removes the original rebuild but adds construction and storage costs; the retained timer includes replacing and dropping the old upper arena. Its additional full L4 allocation is **32 MiB for Falcon-512 and 64 MiB for Falcon-1024 at batch 1024**, not 16/32 MiB: the TwoLimbs representation contributes another row bit. This is temporary proof storage, not a persistent Prepared cache. In the Falcon-512 screen, retention added roughly 8,200 minor faults per proof and increased median process peak RSS by approximately 23–32 MiB depending on worker count and mode.

The production baseline was also profiled before implementation (`baseline-profiles/`): forest medians were 10.760 ms at eight workers and 14.264 ms at 12; JIT rounds were 1.094/0.872 ms and L4 rebuilds 0.272/0.529 ms. This independently identified the limited upside of the two forest experiments. The broader 12-worker slowdown remains: its fold/dense-tail costs and legacy binding construction are substantial.

| PCS initial messages | Control | Four tiles | Two tiles |
|---|---:|---:|---:|
| Falcon-512, 8 workers | 0.940 ms | 0.261 ms | 0.263 ms |
| Falcon-512, 12 workers | 0.946 ms | 0.287 ms | 0.290 ms |

The eight-worker fusion profile moved binding construction from 0.754 to 1.348 ms while moving first-round work into construction; that isolated span is not a whole-binding regression measurement. End-to-end results provide no consistent gain, so fusion stays experimental.

`profile-summary.json` contains stage and worker-time fields; `forest-summary.json` contains the parsed forest breakdown. Nested wall spans overlap and must not be added. Summed worker nanoseconds are not elapsed wall time. The text forest timers are rounded. **Ignore the legacy `BITZ_TRACE` “MB faulted” display**, which assumes 16 KiB pages on this 4 KiB-page Linux host. Benchmark minor faults are counts from `getrusage`; RSS is measured separately. Process peak RSS is a lifetime high-water mark, and retained RSS includes inputs, the allocator and Prepared state. Additional cached-buffer capacity covers only coordinate/word-binding vectors; unavailable metrics are null, not zero.

## Method, correctness and provenance

The host is an AMD Ryzen 9 9950X3D. The pinned worker order is `[6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9]`, with the first N entries used for N workers; the main/auxiliary pool uses CPU 6. Seeds are 42/43/44. Variants run in randomized matched blocks without concurrent builds or tests during latency measurement.

Cold means the first timed commit + proof from a fresh process and Prepared object, seven processes per seed; it does not mean flushed machine caches. Repeated means two warmups followed by seven timed proofs sharing Prepared, one process per seed. Within-process samples are correlated. Preparation, external-input generation, resource sampling and verification are outside prover timing; verification is timed separately. Profiles use one warmup plus three measured proofs per seed. Medians and seed agreement are evidence for this host and workload, not a significance claim or portability guarantee.

The completed runner validation covers **2,559 verified proofs**, including warmups: 672 screen, 864 PCS qualification, 168 profiles, 80 correctness-sweep proofs, seven smoke proofs and 768 production-confirmation proofs. This comprises 1,703 Falcon-512 and 856 Falcon-1024 proofs, with 14 frozen baseline input cases. The correctness sweep covers batches 3/129 at security 100/128 and batch 1024 at security 128, both degrees and 1/8/12/16 workers. The earlier standalone baseline profiles are separate diagnostics and are not included in that count.

For matching degree/batch/security/seed, validation requires identical input digests, source commitments, **complete proof digests and payload sizes**, including across worker counts, configurations and phases. It re-normalizes raw trials, checks verification and requested security, checks worker affinity, rejects missing or stale results, and verifies frozen binary, parser/helper, manifest, command and raw-file hashes. There are no measured proof-size or protocol changes.

Completed native tests: 361 Falcon tests, two large ignored tests run explicitly, 89 BitZ tests, 78 field tests, and the final three streamed-PCS tests (including the production-default policy). Portable checks passed two forest and two PCS tests; nine synthetic runner tests passed. The final library also passes `cargo check --no-default-features --features hybrid`, covering the sequential build. The initial portable PCS build failed because an inherited `FZF_DEFAULT_OPTS` value was invalid Unicode; the recorded retry with that variable removed passed. This was a build-environment failure, not a failed arithmetic test. Test logs, command records and the independent source/runner reviews are retained here.

The frozen baseline is commit `a4585dffa441a00477f5577c20eea9d386a32b35`, executable SHA-256 `321f791c5d7b5c09de5cb3b92ff69327cd0e23272117cc5a69da0d76bb9d235d`. All new experiment measurements use `candidate-v2`, SHA-256 `8db97affb3ba0423ad0fe1301cbfcf411fb8b7639f675fa87f8b641837a5721f`, built with `falcon-hybrid,bench-internals` and `-C target-cpu=native`. Candidate v1 predates private bucket-storage hardening and was not measured. `candidate-v2-source.json`, its saved patch and build logs record source/build provenance; phase manifests separately freeze the final runner and helper hashes. Production promotion changes only the guarded PCS default and is represented by separate `candidate-production-*` artifacts.

## Reproducing and inspecting the evidence

The frozen `phases/*.json` files contain exact commands, configuration flags, seeds, CPU placement, binary hashes and runner/helper hashes. Each `runs/<phase>/<job>/` directory includes raw stdout/stderr, the command, exit status and normalized verified results. `validation.json` records the completed cross-phase audit; `summary.json` contains all medians and per-seed results. Correctness and one-sample smoke phases are not performance qualifications.

Revalidate the evidence in this workspace while the frozen binaries remain available:

```sh
python3 scripts/falcon_prover_optimization.py --output results/falcon-forest-pcs-binding-20261013 --summarize-only
```

`build-candidate.py`, `run-tests.py`, the saved command records and `host.json` document build/test and host settings. No builds or tests overlapped the timing campaigns. The standalone baseline profiling preceded implementation. The final production phase uses the original production binary for its baseline, not the experiment control binary.
