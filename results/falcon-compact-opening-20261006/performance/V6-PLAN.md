# V6 preparation — source frozen, integration pending

V6 sources are frozen at `/tmp/falcon-compact-opening-candidate-v6` (818 files, manifest SHA-256 `4b7df033c3e5a80a6fa495406ae8f5092ca846a2e6d9ae01aee9453af9352594`). The only file changed from V5 is `src/piop/spartan/falcon1024_ct/piop.rs`: an 11-entry compile-time schedule matching the old solver exactly. Both grinding experiments were negative; no production grinding code changed. The root agent owns integration validation before transferring CPU ownership. No V6 benchmark binary or measurements exist yet. [Preparation checks](preparation-v6.json), [exact source audit](v6-source-audit.json).

The exact frozen V4 baseline remains valid: `/tmp/falcon-compact-opening-binaries/baseline-v4`, SHA-256 `24e7406b6ca503f2d4a4b6824e69c21537a78f79492f7b1b9484091525730e55`, with source snapshot `/tmp/falcon-compact-opening-baseline-v4`. Its binary and source-manifest hashes, current compiler, Cargo lockfile, both affinity overlay files, and qualification harness were checked unchanged. No baseline rebuild is needed unless these settings actually change. [Baseline metadata](baseline-build-v4.json).

V5 remains **not qualified**. Its 301 matched qualification pairs, all 264 diagnostic profile proofs, raw logs, and reports remain untouched. Earlier V1/V2/V4 stopped campaigns and unmeasured V3 builds are retained. [V5 diagnostics](v5/DIAGNOSTICS.md).

The finalized source snapshot was created using:

```sh
python3 results/falcon-compact-opening-20261006/performance/freeze_candidate.py snapshot \
  --version v6 \
  --baseline-build results/falcon-compact-opening-20261006/performance/baseline-build-v4.json
```

The canonical benchmark build, after CPU coordination, is:

```sh
RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target/falcon-native \
  cargo build --offline --locked --release --features falcon-hybrid --bench falcon_hybrid \
  --message-format=json-render-diagnostics \
  > results/falcon-compact-opening-20261006/performance/candidate-build-v6.jsonl \
  2> results/falcon-compact-opening-20261006/performance/candidate-build-v6.stderr
```

After successful build completion and source-integrity checks:

```sh
python3 results/falcon-compact-opening-20261006/performance/freeze_candidate.py finalize \
  --version v6 \
  --baseline-build results/falcon-compact-opening-20261006/performance/baseline-build-v4.json
```

The planned snapshot is `/tmp/falcon-compact-opening-candidate-v6`; the immutable executable is `/tmp/falcon-compact-opening-binaries/candidate-v6`; the paired build manifest is `builds-v6.json`.

After integration correctness passes and exclusive CPU ownership is explicitly transferred, run cold diagnostics, then verify exact V6/V5 candidate proof equivalence across all 16 cases: stored proof digests, payloads, source roots, input digests, security terms, grinding records, and protocol versions. V5 cold reference artifacts are in `v5/cold/`. Follow with stage diagnostics and, if all checks pass, a fresh full qualification. All actual global, auxiliary, main, and private verifier masks must pass.

For each phase, run the following with `--phase cold`, then `--phase stages`, then `--phase qualification`:

```sh
python3 scripts/qualify_falcon_one_source.py \
  --campaign compact-opening \
  --builds results/falcon-compact-opening-20261006/performance/builds-v6.json \
  --output results/falcon-compact-opening-20261006/performance/v6 \
  --cpu-affinity 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15 \
  --case-cache /tmp/falcon-one-source-case-cache \
  --cache-reference results/falcon-compact-opening-20261006/performance/cache-reference.json \
  --phase qualification
```

Use the existing fail-stop watcher alongside qualification:

```sh
python3 results/falcon-compact-opening-20261006/performance/watch_campaign.py \
  results/falcon-compact-opening-20261006/performance/v6/qualification
```

All methodology stays unchanged: 16 protocol/security/batch cells, exactly seeds 42–101, 16 global workers on CPUs 0–15, main and serial auxiliary work on CPU 0, one warmup and three measured proofs per process, balanced order, arithmetic means of per-seed medians, paired 20,000-draw bootstrap one-sided 95% upper bounds at most 1.02 for both proving and verification, and strictly smaller mean proof payloads. No host power, security, sampling, or fixture changes. Real grinding remains enabled.

A complete campaign requires all 960 matched pairs and 7,680 verified proofs; a failed complete cell stops it at a pair boundary and preserves every sample. Passing V5 cells are not reused as V6 observations. Estimate 80–90 minutes for full qualification plus approximately 2½ minutes for cold/stage diagnostics, excluding builds and correctness checks. Grinding and idle waits may extend that estimate.
