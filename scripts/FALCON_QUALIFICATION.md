# Current Falcon profile qualification tools

The current matrix is SharedPrime only: degree512/1024, security100/128, batch1/3/32/1024, Auto extension K9/K11, maximum batch1024. Inputs and all real grinding remain part of normal proving. Input generation is outside proof timing.

`qualify_falcon_simplification.py` compares immutable builds using all60 seeds42–101, one warmup and three measured proofs per process, balanced AB/BA/BA/AB order, and16 workers. Every timing observation is a per-seed process median. Both prover and verifier require the paired20,000-resample one-sided95% upper bound on the ratio of arithmetic means to be at most1.02 in every cell. A completed failing cell stops without discarding samples. An incomplete campaign cannot pass.

The payload criterion compares retained fixed buckets at identical public query/rate/fold/row geometry. The two sampled multiproof authentication buckets may vary, and actual total mean/min/max are reported independently. Initial source roots, inputs and stored nonce boundary counts must match. Fresh transcript domains permit different proof/statement digests and nonce values. Both security ledgers are validated independently against the same target.

Only `bench_gate.py`, `bench_support.py` and `bench_statistics.py` are shared Python dependencies. The current tools do not import retired version-specific campaign runners.

## Reproduce the fixture generator

Fixture generation remains defined by `benches/common/falcon_degree_inputs.rs`. The builder reads that canonical file and appends a small reference/publish wrapper in a fresh output directory. It links existing Cargo library artifacts; it does not edit the checkout or modify protocol code. Generation independently produces fresh inputs before looking at the cache, then verifies exact cached bytes, upstream signatures, lossless CT conversion and native preflight. Only public keys/messages/signatures are stored.

Capture artifacts from the same native release build used for the benchmark:

```sh
RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=2 cargo build --offline --locked --release --features falcon-hybrid --bench falcon_hybrid --message-format=json-render-diagnostics > /tmp/falcon-artifacts.jsonl
python3 scripts/build_falcon_fixture_generator.py --source-root "$PWD" --cargo-artifacts /tmp/falcon-artifacts.jsonl --output /tmp/falcon-fixture-generator
python3 scripts/prewarm_falcon_profile_cache.py --generator-method /tmp/falcon-fixture-generator/build-method.json --cache /tmp/falcon-profile-cache --output /tmp/falcon-profile-prewarm --workers 4
```

`build-method.json` records compiler, source, rlib and binary hashes. `prewarm_falcon_profile_cache.py` freshly generates all480 degree×batch×seed fixtures, with at most4 children and one Rayon worker per child. It runs no Falcon prover. Raw child logs, per-fixture records and progress remain in the output directory. `cache-reference.json` is marked complete only after all480 fixtures pass and all final checksums are rechecked. A `STOP` file in the output directory drains current work without launching new children. `--resume` validates existing records/hashes and never overwrites evidence.

Each fixture has a degree-separated filename, an input digest from uncached generation, a BLAKE3 payload checksum and a SHA256 file checksum. The same immutable cache/reference is supplied to both proof builds:

```sh
python3 scripts/qualify_falcon_simplification.py --campaign simplification --builds /tmp/falcon-builds.json --output /tmp/falcon-measurements --phase qualification --cpu-affinity 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15 --case-cache /tmp/falcon-profile-cache --cache-reference /tmp/falcon-profile-prewarm/cache-reference.json
```

The build manifest supplies `binaries.baseline` and `binaries.candidate`, including immutable binary/source-manifest paths and hashes, compiler, lockfile hash, flags, feature set, release profile, architecture, revision, one source-root count and each build's protocol-domain label. Run cold and stage phases first. Finish all compilation, fixture generation and correctness work before performance timings.

The frozen original baseline5f9b23edd currently fails degree512/batch1024 because its tree-count guard incorrectly uses per-tree leaf width. The separate baseline diagnostic and proposed correction must be resolved before a full qualification can succeed; neither this runner nor the prewarmer silently changes that baseline.

## Tests

```sh
python3 -m unittest discover -s scripts -p 'test_qualify_falcon_simplification.py'
python3 -m unittest discover -s scripts -p 'test_prewarm_falcon_profile_cache.py'
python3 -m unittest discover -s scripts -p 'test_build_falcon_fixture_generator.py'
```
