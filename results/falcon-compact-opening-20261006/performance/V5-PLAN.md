# V5 preparation — source frozen, build and measurements pending

The V5 source snapshot is frozen at `/tmp/falcon-compact-opening-candidate-v5` (818 files, manifest SHA-256 `47e3d269adf4ef9f885f43b6c00e4f2643af08c2da635c844e66a3aa31067528`). Only four production files differ from V4; the benchmark overlay and qualification harness are identical. [Exact source audit](v5-source-audit.json). Integration tests and the benchmark build are still pending; no measurements have started.

V5 reuses the exact V4 baseline binary and source snapshot. Both benchmark overlay files, the qualification harness, compiler, Cargo lockfile, and build settings still match. The baseline executable and every frozen source file have been rehashed successfully. No new baseline build is needed: the planned changes affect only the candidate implementation. [Preparation checks](preparation-v5.json), [baseline metadata](baseline-build-v4.json), [placement overlay](benchmark-affinity-overlay-v4.json).

Baseline: `/tmp/falcon-compact-opening-binaries/baseline-v4`, SHA-256 `24e7406b6ca503f2d4a4b6824e69c21537a78f79492f7b1b9484091525730e55`. Source snapshot: `/tmp/falcon-compact-opening-baseline-v4`.

The finalized source snapshot was created using:

```sh
python3 results/falcon-compact-opening-20261006/performance/freeze_candidate.py snapshot \
  --version v5 \
  --baseline-build results/falcon-compact-opening-20261006/performance/baseline-build-v4.json
```

The root agent will coordinate integration tests and the canonical benchmark build. Its final benchmark command is:

```sh
RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target/falcon-native \
  cargo build --offline --locked --release --features falcon-hybrid --bench falcon_hybrid \
  --message-format=json-render-diagnostics \
  > results/falcon-compact-opening-20261006/performance/candidate-build-v5.jsonl \
  2> results/falcon-compact-opening-20261006/performance/candidate-build-v5.stderr
```

After successful build completion, finalization checks the compiler, source snapshot, working files, and Cargo artifact before copying the immutable binary:

```sh
python3 results/falcon-compact-opening-20261006/performance/freeze_candidate.py finalize \
  --version v5 \
  --baseline-build results/falcon-compact-opening-20261006/performance/baseline-build-v4.json
```

This creates `/tmp/falcon-compact-opening-candidate-v5`, `/tmp/falcon-compact-opening-binaries/candidate-v5`, and `builds-v5.json`. Earlier snapshots, binaries, and all stopped qualification data remain untouched.

After correctness passes and exclusive CPU ownership is transferred, run cold and stage diagnostics, then qualification, sequentially. Substitute `cold`, `stages`, and finally `qualification` for `--phase`:

```sh
python3 scripts/qualify_falcon_one_source.py \
  --campaign compact-opening \
  --builds results/falcon-compact-opening-20261006/performance/builds-v5.json \
  --output results/falcon-compact-opening-20261006/performance/v5 \
  --cpu-affinity 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15 \
  --case-cache /tmp/falcon-one-source-case-cache \
  --cache-reference results/falcon-compact-opening-20261006/performance/cache-reference.json \
  --phase qualification
```

Compare the V5 and V4 cold candidate proof digests, payloads, roots, input digests, security headers, and grinding records across all 16 cases. The optimizations are intended to preserve those values. All requested and observed global, auxiliary, main, and private verifier affinity masks remain mandatory. Host governor and power settings remain unchanged.

The timing criterion and sample counts remain fixed: 960 matched pairs across 16 cells, seeds 42–101, one warmup and three measured proofs per process, balanced ordering, arithmetic-mean ratios with one-sided 95% paired bootstrap upper bounds at most 1.02 for proving and verification, and strictly smaller mean payloads. Every complete 60-seed cell is reported immediately. Run the existing watcher alongside qualification:

```sh
python3 results/falcon-compact-opening-20261006/performance/watch_campaign.py \
  results/falcon-compact-opening-20261006/performance/v5/qualification
```

The watcher stops cooperatively after a completed cell fails; otherwise all 960 pairs must finish. No sample exclusion or replacement is allowed.

Expect roughly **80–90 minutes for full qualification**, plus about **2½ minutes for cold and stage diagnostics**, after builds and correctness checks. Extrapolating V4's 32 cold processes to four proofs per process gives about 2,958 seconds of process runtime; 1,920 mandatory one-second idle probes add another 32 minutes, before summary and integrity overhead. The earlier complete one-source campaign using the same sample counts took 84.6 minutes. Grinding and host idle waits can extend this estimate; it is not a runtime guarantee. [Previous complete campaign](../../falcon-one-source-20261006/performance/v3/qualification/manifest.json).
