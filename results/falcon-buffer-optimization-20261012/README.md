The selected change initializes coordinate and word-binding output buffers directly when the Rayon pool has **more than one worker**. One-worker execution keeps baseline initialization. Coordinate baseline/zeroed/direct/reuse, word baseline/direct/reuse, factored initial fusion, factored fold reduction, and combinations were implemented and checked. **Reuse and the two factored arithmetic kernels remain off by default.** No proof protocol changed.

**Production confirmation completed: batch 1024, security 128.** These canonical v9 results compare fresh `falcon-hybrid`-only baseline and final candidate builds, with no experimental selectors. Values are median commit-plus-prove milliseconds; negative changes mean faster. Each cell contains 21 measured proofs.

| Degree | Workers | Cold baseline → candidate, ms | Change | Repeated baseline → candidate, ms | Change |
|---|---:|---:|---:|---:|---:|
| 512 | 1 | 144.608 → 144.857 | +0.17% | 139.245 → 138.920 | -0.23% |
| 512 | 8 | 35.004 → 31.897 | -8.88% | 31.580 → 29.526 | -6.50% |
| 512 | 16 | 35.406 → 31.852 | -10.04% | 32.031 → 29.803 | -6.96% |
| 1024 | 1 | 277.592 → 278.163 | +0.21% | 270.254 → 271.234 | +0.36% |
| 1024 | 8 | 65.118 → 56.820 | -12.74% | 56.182 → 52.147 | -7.18% |
| 1024 | 16 | 61.807 → 53.547 | -13.36% | 54.502 → 50.121 | -8.04% |

All eight 8/16-worker cells and their 24 seed medians improve. Parallel verifier medians improve 8.42–18.58%. One-worker results remain near baseline (-0.23% to +0.36%); no material one-worker speedup is claimed. Per-process peak-RSS median changes range from -0.516 to +0.457 MiB. The final candidate selects worker-dependent defaults and skips binding-cache locks when reuse is disabled. See [analysis-production.json](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/analysis-production.json) and [analysis-production.txt](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/analysis-production.txt).

The experiment qualification and stage tables below use the frozen **candidate-v1 binary with explicit selectors**. They remain distinct from the production-default comparison above. The additional v1 factored-protocol confirmation is complete and reported separately below.

**V1 selector qualification results.** Batch 1024, security 128. Values are median commit-plus-prove milliseconds, candidate control → coordinate-direct + word-direct; negative deltas mean faster. v9 is canonical and v10 is the existing experimental factored-ring protocol. At one worker the direct selectors were deliberately forced on for comparison; the selected final default leaves them off there.

| Degree | Protocol | Workers | Cold control → direct, ms | Change | Repeated control → direct, ms | Change |
|---|---|---:|---:|---:|---:|---:|
| 512 | v9 | 1 | 144.880 → 144.677 | -0.14% | 139.375 → 139.922 | +0.39% |
| 512 | v9 | 8 | 35.007 → 31.983 | -8.64% | 31.543 → 29.431 | -6.70% |
| 512 | v9 | 16 | 35.109 → 31.548 | -10.14% | 31.901 → 29.603 | -7.20% |
| 512 | v10 | 1 | 132.308 → 132.816 | +0.38% | 128.286 → 128.128 | -0.12% |
| 512 | v10 | 8 | 32.659 → 30.632 | -6.20% | 29.089 → 27.907 | -4.06% |
| 512 | v10 | 16 | 32.844 → 30.466 | -7.24% | 30.182 → 28.585 | -5.29% |
| 1024 | v9 | 1 | 277.825 → 278.130 | +0.11% | 270.568 → 271.199 | +0.23% |
| 1024 | v9 | 8 | 65.277 → 57.427 | -12.03% | 56.235 → 51.731 | -8.01% |
| 1024 | v9 | 16 | 61.362 → 53.313 | -13.12% | 54.502 → 49.419 | -9.33% |
| 1024 | v10 | 1 | 250.122 → 251.271 | +0.46% | 241.935 → 241.323 | -0.25% |
| 1024 | v10 | 8 | 59.242 → 53.995 | -8.86% | 52.096 → 47.903 | -8.05% |
| 1024 | v10 | 16 | 57.255 → 51.999 | -9.18% | 51.398 → 47.171 | -8.22% |

All 16 matched cells at 8/16 workers improved, as did all 48 seed medians. Their verification medians improved 1.98–20.10%. One-worker total changes were only -0.25% to +0.46%, providing no reason to change that default. Other parallel worker counts were not measured. Exact seed, verification and memory results are in [analysis-qualify-direct.json](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/analysis-qualify-direct.json) and [qualification-assessment.txt](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/qualification-assessment.txt).

The earlier 16-worker screen tested each switch separately. Both direct variants improved every matched cell and seed. Coordinate-zeroed was competitive in v9 but less consistent in repeated v10. Coordinate reuse regressed repeated degree-1024 v9/v10 by 1.23%/1.57% while retaining 44/about 22 MiB of extra vectors. Word reuse improved repeated v10 by 5.44%/9.20% at degrees 512/1024, but retained 16/32 MiB and did not consistently help v9. Factored initial/fold kernel changes were at most 0.51% in pooled total latency, without compelling consistent gains. The reuse-heavy screen variant named `combined` differs from the qualified `buffers-direct` variant. See [screen-assessment.txt](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/screen-assessment.txt).

**Additional v1 factored confirmation.** A separate completed 16-worker phase repeated explicit `control` versus `buffers-direct` selectors using the frozen v1 `bench-internals` binary. It contains 96 processes and 192 verified proofs, with the same 21 cold/21 repeated samples per configuration. All 12 seed medians improve; verifier medians improve 3.63–14.00%. This confirms the experimental v10 result, separately from final production v9 defaults.

| Degree | Cold control → direct, ms | Change | Repeated control → direct, ms | Change |
|---|---:|---:|---:|---:|
| 512 | 32.871 → 30.196 | -8.14% | 30.116 → 28.388 | -5.74% |
| 1024 | 57.420 → 51.282 | -10.69% | 51.145 → 46.730 | -8.63% |

See [analysis-confirm-factored.json](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/analysis-confirm-factored.json). Frozen-baseline proof comparisons are checked across the complete campaign by the root validator; this confirmation phase itself contains candidate control/direct comparisons.

**Separate stage profiles.** These are traced candidate-v1 results at 16 workers, control → both direct buffers, median wall milliseconds. Each cell has nine measured proofs: three per seed after one excluded warmup. They explain where time changed; the untraced table above supplies end-to-end performance evidence.

| Stage | v9 / 512 | v9 / 1024 | v10 / 512 | v10 / 1024 |
|---|---:|---:|---:|---:|
| Coordinates | 2.861 → 0.712 | 6.066 → 1.460 | 0.607 → 0.265 | 1.017 → 0.485 |
| Coordinate allocation/preparation* | 2.481 → 0.007 | 5.331 → 0.008 | Not split | Not split |
| Collapse | 1.160 → 1.139 | 2.237 → 2.263 | 0.527 → 0.526 | 1.238 → 1.262 |
| Whole ring | 4.872 → 2.669 | 9.641 → 5.240 | 1.810 → 1.486 | 3.420 → 2.756 |
| First word fold | 1.332 → 0.851 | 2.404 → 1.487 | 2.310 → 0.934 | 5.684 → 1.836 |
| Whole binding | 4.689 → 4.452 | 7.584 → 7.308 | 5.313 → 4.123 | 10.009 → 6.600 |

*The canonical allocation field includes output preparation and the small target vector. Nested spans overlap, and separately calculated medians do not sum. Summed worker time is not wall time. The initialization pass nearly disappears, while canonical recurrence worker time rises about threefold and process minor-fault counts stay approximately unchanged. This is consistent with first-touch work moving into parallel writers; it does not show faster recurrence arithmetic or fewer page faults. Some unchanged stages fluctuate: canonical degree-1024 carries rise 0.554 → 0.737 ms, and v10 degree-1024 whole-ring seed 43 regresses despite faster coordinates. Full caveats and raw-span reconstruction are in [profile-assessment.txt](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/profile-assessment.txt) and [binding-profile-assessment.txt](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/binding-profile-assessment.txt).

**Memory and methodology.** Qualification retained **zero additional coordinate and word-binding cached vector capacity**; this metric excludes existing sumcheck scratch. Post-proof RSS median differences versus control were -836 to +212 KiB. Current RSS includes live inputs, Prepared, allocator retention and the output event. Peak RSS is the process-lifetime high-water mark. Minor faults are process-wide counts around commit+prove, including worker threads; they are not allocation bytes. The host page size is 4096 bytes, and no assumed page size is used to derive the counters.

Latency runs were sequential and untraced, pinned with affinity readback, on an AMD Ryzen 9 9950X3D under Linux using `-C target-cpu=native`. Seeds were 42/43/44. Cold uses seven fresh processes and fresh Prepared objects per seed: 21 first proofs per configuration. Preparation and input generation occur before the timer; CPU/filesystem caches are not flushed. Repeated uses two warmups and seven measured proofs per seed in the same process/Prepared: 21 samples but only three process clusters. Per-seed agreement is not a confidence interval. Main/auxiliary execution is pinned to CPU 6; worker order is `[6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9]`. Existing boost/governor settings were retained. See [campaign.json](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/campaign.json), [host.json](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/host.json), and the frozen phase manifests.

**Correctness status after production confirmation and security-100 checks.** [validation.json](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/validation.json) records **3,828 verified proof executions**, including warmups and smoke/security-100 checks, with input/source equality and exact complete-proof digest equality against frozen baselines within each protocol. It verifies commands, raw logs and binary hashes. The separate profiles add **144 verified proofs** in [profiles/validation.json](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/profiles/validation.json), for **3,972 verified proof executions in total**. The production security-100 partial-batch check passed 36 verified proofs at batch 129, both degrees and 1/8/16 workers; those executions are included in the main count. Proof contents and payload sizes remain unchanged within v9/v10; their cross-protocol sizes are separate comparisons. The maximum observed v10 payload growth over matching v9 is 0.5883%.

The final Rust suite passed **355 tests**, plus **two separately run large-batch tests**; three tests were initially ignored, with those two then run explicitly. Field tests passed **23 native and 23 portable** tests, with one existing ignored test in each configuration. Tests cover independent arithmetic references, signed inputs, partial batches, zero/Boolean factors, output initialization and panic safety, buffer reuse/concurrency, full proof/transcript equality and rejection of tampering. Build/test commands and logs are saved alongside [binding-review.txt](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/binding-review.txt), [coordinate-review.txt](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/coordinate-review.txt), [timing-dataflow-review.txt](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/timing-dataflow-review.txt), [final-source-review.txt](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/final-source-review.txt), and [allocation-assembly.txt](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/allocation-assembly.txt). The initial unoptimized suite was interrupted and replaced by an optimized test build retaining assertions/overflow checks.

**Build and source provenance.** Baseline source is `666b546a08af1972b5a40d88787cddb02ccd18b4` on `falcon-poc`. Frozen v1 experiment builds use `falcon-hybrid,bench-internals`; production confirmation uses only `falcon-hybrid`. The [build/source audit](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/build-source-audit.json) confirms fresh field, BitZ and benchmark compilation for all four measured binaries: v1 baseline/candidate and production baseline/candidate. Candidate snapshots and patches distinguish the measured v1 source from final selected defaults: [candidate-source.json](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/candidate-source.json), [candidate.patch](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/candidate.patch), [candidate-v2-source.json](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/candidate-v2-source.json), and [candidate-v2.patch](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/candidate-v2.patch).

| Artifact | SHA256 / status |
|---|---|
| [V1 baseline](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/baseline-binary.json) | `57ee768283ea3ae1ef782d78c5bbb51efc4b25704ffe78370fa225a82d875fa2` |
| [V1 candidate](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/candidate-binary.json) | `226e9f3f5614a721414e227f18f877be9dbe8b162bc915679e39a7e3de77c692` |
| [Production baseline](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/baseline-production-binary.json) | `cc0e1df9cf2feb6d43ced6c0e7c23673c9711b74b4cd1ef10ae594be422dd08a` |
| [Production candidate](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/candidate-production-binary.json) | `321f791c5d7b5c09de5cb3b92ff69327cd0e23272117cc5a69da0d76bb9d235d` |
| [Archived experiment runner](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/runner-experiments.py) | `b8c9e2799a7d85cd6f2db002be7e631542a165610201ec3dc944d9141cfc11b0` |

Two rejected attempts are preserved as diagnostics and excluded from results. The initial smoke explicitly selected word binding through a constructor that bypassed the requested ring selector; strict normalization caught the wrong protocol, the redundant selector was removed, and smoke restarted ([invalid-smoke-selector](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/diagnostics/invalid-smoke-selector)). A shared Cargo target later returned a stale production candidate with **the same hash as the production baseline**. This was caught **before any production measurement**; artifacts were archived under [stale-production-build](/home/john-wu/code/BitZ-pcs/results/falcon-buffer-optimization-20261012/diagnostics/stale-production-build), and the replacement candidate was freshly built in an isolated target directory before the successful production measurements.

The production runner mode uses `--candidate-production --protocols canonical --variants baseline control`, omits experimental selectors for both binaries, and permits unavailable additional-cache metrics from builds without `bench-internals`; RSS/fault checks remain required. Production builds omit the additional cached-vector instrumentation, and the old baseline omits current-RSS/minor-fault metrics. Their null fields are unavailable measurements, not zero; production retained-RSS/fault deltas versus baseline cannot be inferred. The additional factored confirmation uses the frozen v1 experiment binary; it does not claim to measure a final production build exposing v10.
