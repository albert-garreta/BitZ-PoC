# V7 — source frozen, integration pending

V7 sources are frozen at `/tmp/falcon-compact-opening-candidate-v7`: 818 files, manifest SHA-256 `cece40e49492801a1ecf8ffb33304b75c78534da4e86784e4b9994f34fdbaf7d`. The sole change from V6 is `src/utils/blake3x4/avx512.rs`, SHA-256 `29db73960b0485315e041559951c4c117cf64cb6715f473db46d806f3931414a`: exact fixed-round AVX-512 nonce-scanner specialization. No V7 benchmark executable or campaign exists yet. [Source audit](v7-source-audit.json), [preparation metadata](preparation-v7.json).

The exact V4 baseline binary and source snapshot are reused. Compiler, Cargo lockfile, benchmark overlay, affinity policy, qualification harness, and all acceptance criteria match. Baseline SHA-256 remains `24e7406b6ca503f2d4a4b6824e69c21537a78f79492f7b1b9484091525730e55`. No baseline rebuild is needed. [Baseline metadata](baseline-build-v4.json).

The retained V5/V6 data showed that slow seeds did not reproduce, while measured trial times declined within each process. A bounded diagnostic observed driver-reported CPU frequency rising from approximately 4.4 to 5.4 GHz as prover latency fell from approximately 38 to 31 ms. This is an association, not proof of exclusive causality. [Frequency evidence](v6/frequency-abba/README.md), [retained-data comparison](v6/V5-COMPARISON.md).

A scoped `powerprofilesctl launch --profile performance` probe was denied by the host D-Bus service with `org.freedesktop.DBus.Error.AccessDenied` for `org.freedesktop.UPower.PowerProfiles.hold-profile`. The profile and all 32 policy snapshots before/after were identical: `balanced`, governor `powersave`, EPP `balance_performance`. No power-policy change occurred. This was a host service authorization denial, not an automatic approval-review rejection. V7 therefore retains the same balanced environment as V4, V5, and V6. [Exact command, error, and snapshots](profile-control/probe.json).

The source snapshot was created with:

```sh
python3 results/falcon-compact-opening-20261006/performance/freeze_candidate.py snapshot \
  --version v7 \
  --baseline-build results/falcon-compact-opening-20261006/performance/baseline-build-v4.json
```

After root integration validation and CPU coordination, build and finalize:

```sh
RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target/falcon-native \
  cargo build --offline --locked --release --features falcon-hybrid --bench falcon_hybrid \
  --message-format=json-render-diagnostics \
  > results/falcon-compact-opening-20261006/performance/candidate-build-v7.jsonl \
  2> results/falcon-compact-opening-20261006/performance/candidate-build-v7.stderr

python3 results/falcon-compact-opening-20261006/performance/freeze_candidate.py finalize \
  --version v7 \
  --baseline-build results/falcon-compact-opening-20261006/performance/baseline-build-v4.json
```

This produces `/tmp/falcon-compact-opening-binaries/candidate-v7` and `builds-v7.json`. Once integration passes and exclusive CPU ownership is transferred, run cold diagnostics, compare the V7 candidate against all 16 V6 candidate cold proofs (stored digest, payload, source root, input, security terms, grinding records, protocol), then run stages and a fresh qualification. All actual global, auxiliary, main, and private-verifier affinity masks remain mandatory.

Use `--phase cold`, then `stages`, then `qualification`, sequentially:

```sh
python3 scripts/qualify_falcon_one_source.py \
  --campaign compact-opening \
  --builds results/falcon-compact-opening-20261006/performance/builds-v7.json \
  --output results/falcon-compact-opening-20261006/performance/v7 \
  --cpu-affinity 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15 \
  --case-cache /tmp/falcon-one-source-case-cache \
  --cache-reference results/falcon-compact-opening-20261006/performance/cache-reference.json \
  --phase qualification
```

Run the fail-stop watcher alongside qualification, retaining its output in a log so monitoring can recover after a conversation interruption:

```sh
python3 results/falcon-compact-opening-20261006/performance/watch_campaign.py \
  results/falcon-compact-opening-20261006/performance/v7/qualification \
  > results/falcon-compact-opening-20261006/performance/watcher-v7-console.log 2>&1
```

Criteria remain: all 16 cells, seeds 42–101, one warmup and three measured proofs per process, balanced ordering, 16 workers, arithmetic means of per-seed medians, paired 20,000-draw bootstrap one-sided 95% upper bounds at most 1.02 for both prover and verifier, and strictly smaller mean payloads. Real grinding remains enabled with unchanged difficulty and proof nonces. No sampling, security, affinity, harness, or host-setting changes. Every completed cell is reported; a failing complete cell stops at a pair boundary. Full completion requires 960 pairs and 7,680 verified proofs.

All earlier failed campaigns, bounded profiles, frequency observations, and negative grinding experiments remain preserved. None is replaced or relabeled. Estimate roughly 80–90 minutes for a passing full campaign plus 2½ minutes cold/stage diagnostics, excluding builds and correctness checks; grinding and idle waits can extend the duration.
