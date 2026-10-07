# Combined Falcon preparation

`PreparedFalconAlgebraic::commit_from_s2(statement, s2)` derives centered `s1`, checks the exact norm, retains slack and the ring quotient, and commits through the existing packing path. It computes one integer product and one norm per signature. The supplied-witness `commit` API still validates both components, including valid non-centered `s1`. The statement, verifier, packing, and proof layout are unchanged; there is one initial witness commitment, with the existing recursive PCS commitments unchanged.

See [REPORT.md](REPORT.md) for all measured results and [validation.json](validation.json) for checks. The algebraic tests passed with 1 and 16 Rayon workers (23 passed, 1 pre-existing large test ignored in each run). The benchmark supplied coverage for 32/1,024 signatures at both security targets: all 240 proofs verified. The `falcon-hybrid` feature implies `parallel`, so a build without parallelism is not supported by this module.

## Timing schema

The example now emits `preparation_mode: "combined"` and `witness_commit_ms`, replacing `witness_ms` and `commit_ms`. This includes witness derivation, validation, packing, and initial commitment. The total, prove, and verify timing boundaries are preserved. Baseline preparation is the sum of its two phase times per sample before taking medians. Historical reports and runners were left unchanged.

## Reproduction

The baseline was preserved before editing at `/tmp/falcon-combined-benchmark/falcon_algebraic_baseline`, SHA256 `21107dc244838f2d0aae8059329c8e5dee908b376c137413edd567e411456397`. All baseline source fingerprints match the prior 1,024-signature run. `implementation.patch` records the exact three-file change against that working tree, which includes earlier uncommitted work on base commit `f22ec52c4c328499526dc3a02d938886f80d5cd5`.

Build the candidate from the repository root:

```sh
CARGO_TARGET_DIR=/tmp/falcon-simplify-candidate-target CARGO_BUILD_JOBS=4 RUSTFLAGS='-C target-cpu=native' cargo build --offline --locked --release --features falcon-hybrid --example falcon_algebraic
```

Run the saved comparison runner with an output directory that does not already exist, after stopping other builds and benchmarks:

```sh
python3 results/falcon-combined-preparation-20261007/run_comparison.py \
  --baseline /tmp/falcon-combined-benchmark/falcon_algebraic_baseline \
  --candidate /tmp/falcon-simplify-candidate-target/release/examples/falcon_algebraic \
  --out /tmp/falcon-combined-rerun
```

The runner compares batches 32 and 1,024, security targets 100 and 128, and 1, 2, 4, 8, 16 threads. Each configuration uses seed 42, one warm-up, and five measured trials. It alternates the first variant for adjacent before/after runs, checks matching inputs and proof payload breakdowns, and records commands, affinities, source hashes, and binary hashes. It expects the recorded Linux CPU topology and benchmark schemas; use `--root` if the checkout moved. Executables and input caches remain outside the repository.
