# SHA-256 with BitZ benchmarks

Run a small campaign with both existing BitZ workloads, one Rayon worker,
three measured repetitions and one verified warmup per configuration:

```sh
python3 scripts/run_sha256_bitz_bench.py
```

The default shapes are `7 8 10`: 128, 256 and 1,024 compressions. Each
workload/shape runs in a fresh process. The runner builds both benchmarks
with `cargo bench --locked`, discovers their executables from Cargo JSON,
and invokes those executables directly. It selects only the normal BitZ
features plus `unchecked`; no comparison backend is enabled.

For a minimal run using cached dependencies:

```sh
python3 scripts/run_sha256_bitz_bench.py --offline \
  --workload compressions --shapes 7 --reps 1 --threads 1 \
  --out-dir bench_results/sha256-smoke
```

For every power of two from 16 through 1,024 independent compressions:

```sh
python3 scripts/run_sha256_bitz_bench.py --offline \
  --workload compressions --shapes 4 5 6 7 8 9 10 --threads 1
```

`--workload` accepts `compressions`, `chain` or `both`. Independent
compression exponents are 4 through 16; the chain supports 7 through 16.
Batches of 16, 32 and 64 independent compressions use the packed inner
sumcheck; larger power-of-two batches use the direct product opening.
`--lambda` defaults to `100` and also accepts
`128` or `sha128-reference-schedule`. `--seed` accepts decimal or `0x` hex.
`--checked` omits the `unchecked` feature. `CARGO_HOME`, `CARGO_TARGET_DIR`
and existing compiler flags are inherited; `RUSTFLAGS` defaults to
`-C target-cpu=native` when unset. Reuse the same flags, features, thread
count and security profile when comparing campaigns.

The independent workload proves arbitrary public `(state, block, output)`
compressions. The chain workload starts from the standard IV and proves
`H[i+1] = Compress(H[i], block[i])`, with public blocks and final chaining
state. The intermediate states are witness. Neither workload adds or
constrains SHA-256 message padding; the chain is a raw block chain, not a
complete padded message hash. Its `message_bytes` field counts raw block
bytes, `64 * compressions`.

The library API is exported from `bitz::piop::spartan`. The existing
[compression benchmark](../benches/sha256_compressions.rs) contains a
complete example of this sequence:

```text
prepare_sha256_compression_batch(k) -> prepared
sha256_compression_configs(&prepared) -> prover/verifier configs
generate_sha256_compression_witnesses(&prepared, &inputs) -> witness
Sha256CompressionStatement::new(input, output) -> one statement per input
commit_sha256_compression_witness_with_config(...) -> commitment + prover hint
prove_sha256_compressions_with_config(...) -> proof
verify_sha256_compressions_with_config(...) -> verification result
```

Use separate fresh `Blake3Transcript`s for proving and verification. The
verifier receives the prepared public relation, public statements,
commitment and proof. The [chain benchmark](../benches/sha256_chain.rs)
uses the corresponding `prepare_sha256_chain_batch`,
`generate_sha256_chain_witnesses`, `commit_sha256_chain_witness_with_config`,
`prove_sha256_chain_with_config` and `verify_sha256_chain_with_config` APIs;
its single statement comes from `witness.statement()`.

Timing boundaries:

- `witness_ms`: circuit replay, source/assignment bit packing and public
  statement materialization; the chain also computes native chaining states.
- `setup_ms`: one-time public relation preparation and PCS configuration.
- `prove_ms`: commitment through the finished BitZ proof, excluding witness
  generation, setup and verification. `s1_commit_ms` is included in it.
- `verify_ms`: verification of that proof. Every warmup and sample is verified.
- `witness_to_proof_ms`: the median of each sample's
  `witness_ms + prove_ms`, calculated from the recorded sample timings.

Results go into a new timestamped directory under `bench_results`, or the
new path supplied with `--out-dir`; existing output paths are rejected.
The directory contains `build.log`, one full log per workload/shape,
`summary.csv` preserving all existing `RESULT` fields, `samples.csv`, and
`metadata.json` with CPU, OS, git revision/status, Rust versions, features,
seed, compiler flags and relevant environment settings. The runner clears
SHA-specific shape/layout/trace overrides and interval tracing to select
the standard benchmark path; other recorded optimization knobs are inherited.

A failed process, missing result, or incorrect verified-sample count fails
the campaign and preserves its logs and previously completed CSV rows.
The default three samples are a quick baseline; use more repetitions for
performance conclusions. Benchmarks run sequentially, so avoid other CPU
heavy work during measurement. The historical `sha256_profile_report.py`
targets a different full-range tracing campaign and is not used here.
