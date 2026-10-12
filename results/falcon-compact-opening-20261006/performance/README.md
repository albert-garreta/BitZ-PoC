# Falcon compact-opening qualification

The comparison baseline is commit `9a401286c98db041762b97f48cfe4f1a8d85ce7c`. This campaign evaluates the zero-lane and final-message encodings against the already qualified one-source implementation. The unpinned V1 and V2 campaigns are **not qualified**; their inconclusive verifier results exceeded the accepted confidence-bound limit. V1 stopped after 78 pairs and 624 verified proofs; V2 stopped after 122 pairs and 976 verified proofs. All samples and final integrity checks are preserved. See the [V1 report](v1/qualification/REPORT.md), [V1 diagnostics](v1/DIAGNOSTICS.md), and [V2 report](v2/qualification/REPORT.md).

V4 is also **not qualified**: it stopped after 61 pairs and 488 verified proofs when the complete Native/100/batch-1 cell exceeded both timing confidence-bound limits. All explicit CPU-mask checks passed, and final source/binary/cache integrity passed. See the [V4 report](v4/qualification/REPORT.md) and [V4 diagnostics](v4/DIAGNOSTICS.md).

V5 is **not qualified**: it stopped after 301 matched pairs and 2,408 verified proofs when the complete Native/128/batch-1 prover bound exceeded 1.02. All four Native/100 cells passed. Final source/binary/cache and CPU-mask checks passed. Two bounded attribution profiles are preserved separately and do not replace qualification. [V5 report](v5/qualification/REPORT.md), [V5 diagnostics](v5/DIAGNOSTICS.md).

V5 includes substantive candidate optimizations: a contiguous Keccak coefficient kernel, in-place opening-prefix compaction, and public-exponent width buckets with a public generator-comb cache. Integration checks passed (134 Falcon tests, 16 opening tests, 10 distinct generic-client tests, and the explicit 28-profile matrix). It reuses the exact V4 baseline, compiler, affinity overlay, harness, and timing criterion. Its 818-file source snapshot changes only four production files relative to V4. The candidate build and all 16 cold/stage diagnostic pairs passed; candidate proof digests and payloads match V4 exactly. [Source audit](v5-source-audit.json), [prepared commands and duration estimate](V5-PLAN.md).

V6 is **not qualified**: it stopped after 301 matched pairs and 2,408 verified proofs when the Native/128/batch-1 prover confidence bound reached 1.05053. All four Native/100 cells passed; the failed cell's verifier and payload gates passed. All samples and final integrity checks are preserved. [V6 report](v6/qualification/REPORT.md), [V6 diagnostics](v6/DIAGNOSTICS.md).

V6 contains the compile-time PIOP schedule change; all other V5 production files are unchanged, including grinding. Integration passed 135 Falcon tests, 16 opening tests, 10 distinct generic-client tests, and the explicit 28-profile matrix. It uses the exact V4 baseline and the unchanged affinity, harness, sample counts, timing gates, security, and power settings. The candidate build, all cold/stage cases, and exact V5/V6 cold proof equivalence passed. [V6 source audit](v6-source-audit.json), [V6 plan](V6-PLAN.md).

V7 is active with the exact fixed-round AVX-512 nonce-scanner optimization. This is the sole production change from V6. Integration passed the scanner and grinding tests, 135 Falcon tests, 16 opening tests, 10 distinct generic-client tests, and the explicit 28-profile matrix. The V4 baseline, balanced power profile, affinity, harness, 60 seeds, one warmup/three measured samples, security settings, and 2% timing gates remain unchanged. The scoped performance-profile probe was denied by the host D-Bus service and changed no policy. V7 build, all cold/stage cases, and exact V6/V7 cold proof equivalence passed. The full qualification is active and has no final result yet. [Source audit](v7-source-audit.json), [V7 plan and exact policy-probe evidence](V7-PLAN.md).

Performance and correctness claims for V7 apply to the immutable `/tmp/falcon-compact-opening-candidate-v7` snapshot and binary SHA-256 `f784b4cdf4a26b657f5390da67c3c0bbb46aa92bb0418cd001b39f486a4c0677`. Concurrent workspace refactoring has changed 22 frozen files and added four code files; those current changes are preserved and are outside this qualification. [Working-tree divergence audit](v7/working-tree-concurrent-audit.json). A read-only two-second host CPU observation found no compiler/build processes or sustained heavy load at that time; earlier/transient overlap cannot be ruled out. [CPU observation](v7/host-cpu-overlap-audit.json).

V3's sources and built binaries are preserved **without any timing measurements**. A placement audit required explicit treatment of auxiliary pools before qualification began.

V4 keeps V2's production code and explicitly changes benchmark placement for **both** revisions: global worker indices 0–15 are pinned to CPUs 0–15, one hardware thread per physical core, and the main thread is pinned to CPU 0. The auxiliary pool retains 16 workers and its existing dispatch, with all workers pinned to CPU 0 for the serial Keccak fold job; all actual parallel work on the measured Falcon path retains the global 16-core pool. The private `flock-verify` thread inherits CPU 0 and its actual singleton mask is checked after every successful trial, outside elapsed timers. The opt-in policy is `linux-x86_64-falcon-serial-aux-pinned-v1`; other platforms are rejected when this option is selected.

The machine has unlike 96 MiB and 32 MiB L3 domains; unpinned processes could migrate between them. The governor and boost settings remain unchanged. The fixed 60 seeds, one warmup, three measured proofs, balanced ordering, and 2% confidence-bound gates remain unchanged. V4 is a fresh campaign in a documented environment, not an extension or relabeling of either stopped campaign. [Machine audit](machine-audit.json), [identical benchmark overlay](benchmark-affinity-overlay-v4.json), [production equivalence](v4-production-equivalence.json), [seven affinity tests](../validation/affinity-tests-v4.log), [20 harness tests](harness-v4-tests.log).

The [baseline build metadata](baseline-build.json), [source manifest](baseline-source-manifest.json), and [binary reuse proof](baseline-reuse-proof.json) preserve the baseline. All 574 build input files match the previous frozen V3 build byte for byte, including embedded assets, the Cargo lockfile, and toolchain. The exact Cargo manifest retains the pre-existing [example registration](baseline-example-overlay.patch); it does not change the benchmark target. The fresh snapshot contains 817 files, including the existing untracked examples needed to reproduce that manifest. Results directories are excluded from source snapshots.

The reused binary is `/tmp/falcon-compact-opening-binaries/baseline-9a401286`, SHA-256 `2753f4b6afdc0ccbc73da9924411509faa6ede32a7502938c88d5b9f657c4efc`. Its frozen sources are in `/tmp/falcon-compact-opening-baseline-9a401286`. [freeze_baseline.py](freeze_baseline.py) records how the snapshot and equivalence evidence were produced. The compiler is Rust 1.98.1; both builds use the locked dependency set, release profile, `falcon-hybrid`, and `RUSTFLAGS='-C target-cpu=native'`.

For V4, the baseline is rebuilt from `/tmp/falcon-compact-opening-baseline-v4`, with only `benches/falcon_hybrid.rs` and the new `benches/common/falcon_affinity.rs` changed. Every other original file is unchanged. The candidate uses the identical two benchmark files, while all 517 production/build-configuration files match frozen V2. Both fresh builds are recorded separately; all earlier binaries remain intact. [Baseline snapshot procedure](freeze_affinity_baseline.py), [candidate snapshot procedure](freeze_candidate.py), [benchmark overlay patch](benchmark-affinity-overlay-v4.patch).

Qualification keeps the existing timing criterion:

- Native and SharedPrime; 100-bit and 128-bit security; batches 1, 3, 32, and 1,024; 16 threads.
- Exactly seeds 42 through 101, one warmup and three measured proofs per process, balanced AB/BA/BA/AB process order.
- A seed's process median is one observation. Each metric compares the arithmetic means across seeds.
- The paired 20,000-draw bootstrap's one-sided 95% upper bound must be at most 1.02 for both total proving and verification time in all 16 cases.
- Mean proof payload must be strictly smaller in every case. There is no new 20% reduction requirement relative to `9a401286`; the original one-source preset retains its original requirement relative to `51405193`.
- Every proof must verify. Input digests, source roots, security terms, and grinding diagnostics must match each pair. Stored proof digests may change because the representation changes. All grinding remains enabled.
- A complete campaign contains 960 matched pairs, 1,920 processes, and 7,680 verified proofs including warmups. Diagnostic and incomplete campaigns cannot pass.

All 240 existing deterministic fixtures were rehashed successfully. [cache-reference.json](cache-reference.json) is an unchanged copy of the original generated-reference manifest; its SHA-256 is `dd124d12aef75490d5dbb82dced166d4229deeda0e60d55b1a3802883327ad1c`. The fixture directory is `/tmp/falcon-one-source-case-cache`. Setup and cache validation occur outside the timed regions. Each process validates its input digest against the reference, and final integrity checks rehash every cached fixture, executable, and frozen source file.

After a candidate has passed correctness checks, been frozen into its versioned build manifest, and received exclusive CPU ownership, run the following from the repository root. Change `--phase` to `cold` and `stages` first. This is the V4 command; the stopped V1 and V2 output directories must not be reused.

```sh
python3 scripts/qualify_falcon_one_source.py \
  --campaign compact-opening \
  --builds results/falcon-compact-opening-20261006/performance/builds-v4.json \
  --output results/falcon-compact-opening-20261006/performance/v4 \
  --cpu-affinity 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15 \
  --case-cache /tmp/falcon-one-source-case-cache \
  --cache-reference results/falcon-compact-opening-20261006/performance/cache-reference.json \
  --phase qualification
```

Cold runs cover all 16 cases and will establish whether both encodings reduce payload at the smallest geometries. The full qualification makes that assessment using mean bytes across all 60 seeds. Stage instrumentation is disabled for qualified timings. A `STOP` file in the current phase directory stops cooperatively after the active pair, preserves all evidence, and records the campaign as not qualified. Resume requires unchanged binaries, frozen sources, configuration, harness, and artifacts; a different candidate requires a new campaign directory.

The harness tests cover the distinct baseline/root requirements, the unchanged one-source size gate, strict compact-opening size comparisons using mean bytes, transcript-related invariants, and cooperative stopping. [preparation.json](preparation.json) records preparation checks.
