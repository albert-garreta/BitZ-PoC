# V2 correctness validation

These checks cover the factored Keccak coefficient walker and the complete
Falcon integration. Runtime qualification is recorded separately; correctness
tests do not establish the 2% acceptance gate.

## Root integration

The locked native release build passed the following checks, using sixteen
Rayon threads and one test thread:

| Checks | Passed | Evidence |
| --- | ---: | --- |
| Falcon modules, including exact security ledgers | 131 | [log](correctness.log) |
| Both protocols, both targets, batches 1/2/3/7/8/9/1024 | 28 proofs | [log](profile-matrix.log) |
| Joint and existing shared openings | 9 | [log](all-opening-tests.log) |
| Existing generic hybrid clients | 10 | [log](generic-hybrid-tests.log) |
| Upstream Falcon interoperability and input cache | 4 | [log](upstream-tests.log) |

The normally ignored full profile matrix was explicitly run. The two unrelated
ignored root kernel timing tests were not selected. Complete proof equality
with the first one-source build is checked by the separate cold benchmark.

## Results

- [Focused Keccak tests](vendor-keccak-tests.log): **14 passed, 0 failed,
  6 ignored**. The ignored tests comprise five existing standalone PCS tests
  and the separately executed timing benchmark.
- Exact old/new comparison covers all 65,536 output coefficients for three
  arbitrary field-weight tables, each with alpha zero, one, and random.
  The test-only reference retains the original separate A/B recurrences,
  eleven-preimage transpose scatters, and expanded round constants.
- The factored transpose matches the original scatter on **all 1,600 basis
  vectors**. An independent forward homogeneous-circuit check also passes
  with constant wire zero and one.
- [Broader Keccak/Keccak3 tests](vendor-keccak-extended-tests.log): **18 passed,
  2 failed, 9 ignored**. Keccak3's `fold_matches_witness_consistency` passes,
  covering its use of the shared transpose helper. Both failures occur before
  coefficient folding: existing `all_zero_witness_rejected` and
  `prove_fast_roundtrip` request `log_batch_size=6`, while their embedded
  m=22 Fast PCS fixture has `initial_k=4`. Those fixtures were not changed.
- [Isolated microbenchmark](vendor-keccak-microbenchmark.log): **1 passed**;
  eight warmup pairs followed by 64 alternating old/new pairs. Median time
  fell from **1.280228 ms to 0.177050 ms** per coefficient fold (ratio
  **0.138296**, saving **1.103179 ms**). This measures the coefficient walker,
  not full proving or verification, and does not assert whole-proof byte
  identity; root integration performs that check separately.

## Build and reproducibility

The isolated vendor workspace was `/tmp/falcon-vendor-workspace/flock-mod`,
with its sibling `field` dependency copied from the repository. This avoids
changing the repository's existing vendor lockfile. The target directory was
`/tmp/falcon-vendor-tests`. Toolchain: `rustc 1.98.1 (48a229cea 2026-09-01)`
and `cargo 1.98.1 (797e8a9bc 2026-08-05)`.

The native release build used `RUSTFLAGS='-C target-cpu=native'`,
`CARGO_BUILD_JOBS=2`, and `RAYON_NUM_THREADS=4`, with the same isolated setup
as the preceding vendor validation. The `unsound-challenger` feature is
needed to compile existing test code using `RandomChallenger`; it does not
change production features. The prefix compatibility tests use `FsChallenger`.

```sh
CARGO_TARGET_DIR=/tmp/falcon-vendor-tests CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=4 RUSTFLAGS='-C target-cpu=native' cargo test --manifest-path /tmp/falcon-vendor-workspace/flock-mod/Cargo.toml -p flock-prover --features unsound-challenger --release --lib --offline r1cs_hashes::keccak -- --test-threads=1
RAYON_NUM_THREADS=4 /tmp/falcon-vendor-tests/release/deps/flock_prover-01d4cb92b5c679ae r1cs_hashes::keccak::tests:: --test-threads=1
RAYON_NUM_THREADS=4 /tmp/falcon-vendor-tests/release/deps/flock_prover-01d4cb92b5c679ae r1cs_hashes::keccak::tests::combined_walker_microbenchmark --exact --ignored --nocapture --test-threads=1
```

The first command records the broader run and exits unsuccessfully because
of the two fixture errors described above. The focused rerun passes.
The benchmark ran on an otherwise idle CPU after the qualification worker
stopped. Its production code matches the final source; the subsequent edit
only removed an unnecessary `mut` in the test benchmark closure.

Final repository and isolated-copy SHA-256 of
`vendor/flock-mod/crates/flock-prover/src/r1cs_hashes/keccak.rs`:
`c259dac7568caf8642098badc019b85e0f077cc7e5076aa75e9a52a9a67b4f77`.
