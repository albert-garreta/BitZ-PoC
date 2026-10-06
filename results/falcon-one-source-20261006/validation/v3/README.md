# V3 correctness validation

This validates grouped stripe transposition for scalar partial Keccak batches
and the single masked wiring gather. No dummy Keccak computations were added.
Whole-proof identity and runtime qualification are recorded separately.

## Root integration

The locked native release build passed all 133 Falcon module tests and the
explicit 28-proof matrix covering both protocols, both security targets, and
batches 1/2/3/7/8/9/1024. The new gather tests compare every prefix limit against
a dense non-Boolean reference and compare complete joint sumcheck proofs with
both the previous subcube representation and a dense prover.

| Checks | Passed | Evidence |
| --- | ---: | --- |
| Falcon modules and exact security ledgers | 133 | [log](correctness.log) |
| Complete profile matrix | 28 proofs | [log](profile-matrix.log) |
| Joint and existing shared openings | 9 | [log](all-opening-tests.log) |
| Existing generic hybrid clients | 10 | [log](generic-hybrid-tests.log) |
| Upstream Falcon interoperability and input cache | 4 | [log](upstream-tests.log) |

Root tests used sixteen Rayon threads and one test thread. The normally ignored
profile matrix was explicitly run; unrelated kernel timing tests were not.
The `falcon_cost_study` and `falcon_size_breakdown` examples also type-check.

The pre-existing untracked `proof_size_comparison` prototype still references
three unavailable APIs: `CommittedFalconHybrid::{committed_source_bits,
encoded_source_bits}` and `Sha256EcdsaProof::size_breakdown`. Those APIs are
also absent from the frozen baseline. Its root-count expression was adapted to
the single-root API, but its broader type-check remains unsuccessful.

## Correctness

[Focused vendor tests](vendor-keccak-tests.log): **15 passed, 0 failed,
7 ignored**. The ignored tests are five existing standalone PCS tests and two
separately selected timing benchmarks. The existing PCS fixture limitations are
recorded in [V2 validation](../v2/README.md).

The new test
`partial_chain_stripes_match_scalar_oracle_and_real_constraints` covers these
`(live signatures, capacity, permutations)` shapes:

```text
(1,1,16), (1,2,4), (2,2,4), (3,4,4), (3,4,16), (4,4,16),
(5,8,4), (7,8,4), (9,16,16), (13,16,4), (16,16,4)
```

It poisons recycled source buffers with arbitrary nonzero data, compares every
stripe byte against the old per-set-bit scalar oracle, checks A/B images using
the real homogeneous circuit equations, checks every A*B=C row, checks chain
inputs/outputs and live constants, and requires inactive source/A/B/output
buffers to be zero. Cases include groups spanning permutations when capacity
is below eight, a SIMD group followed by a scalar tail, and full SIMD batches.
The earlier exact coefficient-walker and all-1,600-basis-vector tests also pass.

## Isolated stripe timings

[Microbenchmark](vendor-keccak-microbenchmark.log): **1 passed**. Each case uses
eight warmup pairs followed by 64 alternating old/new pairs on an otherwise
idle CPU. The old emitter reads contiguous scalar witness-word buffers, as it
did in production; it is not penalized by artificial gathers from the packed
source. The new emitter gathers packed word rows and calls the existing
eight-word transpose. Input preparation and clearing the destination are
outside the timed interval for both methods.

| Live | Capacity | Permutations | Old median ms | New median ms | Saved ms |
|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 16 | 0.174861 | 0.003136 | 0.171725 |
| 1 | 2 | 4 | 0.034415 | 0.001658 | 0.032757 |
| 3 | 4 | 16 | 0.557701 | 0.015244 | 0.542458 |
| 3 | 4 | 4 | 0.126920 | 0.003176 | 0.123744 |
| 9 | 16 | 16 | 0.176174 | 0.035762 | 0.140412 |

The two batch-three slab measurements save approximately **0.666202 ms** in
total. These measure stripe emission alone, not complete generation, proving,
or verification; they do not establish the whole-Falcon acceptance gate.

## Reproduction

Same isolated native release setup as V2: workspace
`/tmp/falcon-vendor-workspace/flock-mod`, sibling `field` dependency, target
`/tmp/falcon-vendor-tests`, `rustc 1.98.1 (48a229cea 2026-09-01)`,
`cargo 1.98.1 (797e8a9bc 2026-08-05)`. The repository vendor lockfile is
unchanged. `unsound-challenger` is needed to compile existing test-only imports;
production features are unchanged, and prefix compatibility uses `FsChallenger`.

```sh
CARGO_TARGET_DIR=/tmp/falcon-vendor-tests CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=4 RUSTFLAGS='-C target-cpu=native' cargo test --manifest-path /tmp/falcon-vendor-workspace/flock-mod/Cargo.toml -p flock-prover --features unsound-challenger --release --lib --offline r1cs_hashes::keccak::tests:: -- --test-threads=1
RAYON_NUM_THREADS=4 /tmp/falcon-vendor-tests/release/deps/flock_prover-01d4cb92b5c679ae r1cs_hashes::keccak::tests::partial_chain_stripes_microbenchmark --exact --ignored --nocapture --test-threads=1
```

Final SHA-256 of
`vendor/flock-mod/crates/flock-prover/src/r1cs_hashes/keccak.rs`:
`cc198b020e8e46cee6bdd6c8bf55f93e8a2ffbfba7c0d4a0cf1a95c0657899a1`.
