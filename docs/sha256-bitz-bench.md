# SHA-256 compressions with BitZ

The benchmark [`sha256_compressions`](../benches/sha256_compressions.rs)
proves `2^k` independent SHA-256 compressions `output = Compress(state,
block)` in one BitZ-SNARK proof, for `k` from 7 to 16. By default the input
states, blocks and outputs are public. With `BITZ_SHA_PRIVATE_IO=1` nothing
is public: the proof only says that the committed witness holds `2^k` valid
compressions, which is the statement Flock's `sha2_proof` benchmark proves.
No message padding is added or constrained.

## What is proved, and how

Each compression is arithmetized as an instance of the CM relation: a vector
`f` of 7,144 committed bits per compression (plus one shared constant), a
derived vector `h = M·f` of 16,360 integer cells per compression, and 184
integer constraints per compression that are linear in `h`, except for
`Ch`. We do not lift the two words `e ⊕ f` and `e ⊕ g` of every round to
integers. Instead, `Ch` enters each constraint as `2 Ch = 2 g + Σ_i 2^(i+1)
e_i (f_i − g_i)`, i.e. as one product ``slot'' per round and bit, 2,048 per
compression. This brings `h` from 20,457 to 16,361 cells, i.e. from a `2^15`
to a `2^14` stride per compression.

The proof runs as follows:

1. Commit `f`.
2. Sample the runtime prime `q` from the transcript.
3. Collapse the constraints with a local-row point `ξ` and the compressions
   with an instance point `η`, batching the public input/output equalities
   and the shared constant behind two more challenges. The product slots
   are then reduced by a sumcheck over the instance and slot variables
   (`src/piop/spartan/protocol/product.rs`); its three terminal claims are
   folded into one rank-one linear claim on `h`.
4. Prove that claim with the BitZ PCS.

For the last step, the default opening (`BITZ_OPENER=block`) commits `f`
in a block layout: compression `i`'s bits sit in their own `2^13`-cell
block, so the map from the committed grid to `h` is exactly `Id ⊗ M` and
the GKR's exit claim on `h` transposes to a rank-one claim on the commitment
(`src/bitz/block.rs`). BitZ runs its fold and GKR over `h` at its split
`t = ⌈0.6 n⌉ − 1`, at least 14 (`n = k + 14`; `BITZ_SHA_BLOCK_T=<t>`
overrides it).
`BITZ_OPENER=virtual` commits the packed `f` instead and transposes the
claim through the dense virtual map, which costs one weight per committed
bit for both prover and verifier. The explicit-split
(`BITZ_SHA_OPENING_T`) and assignment-row-sized (`BITZ_SHA_MNUMROWS_LOG2S`)
modes run the virtual opening only.

## Running a campaign

`scripts/run_sha256_bitz_bench.py` builds the benchmark once and runs one
fresh process per (thread count, Ligerito profile, batch size), each with
one verified warm-up and `--reps` verified samples:

```sh
python3 scripts/run_sha256_bitz_bench.py \
  --shapes 13 14 15 16 --threads 1 10 --reps 5 \
  --profiles custom:1:4 custom:3:4 --private-io --rss \
  --out-dir bench_results/sha256-compressions
```

`--profiles` takes `BITZ_LIG_PROFILE` values (`custom:1:4` is rate 1/2,
`custom:2:4` rate 1/4, `custom:3:4` rate 1/8). `--private-io` sets
`BITZ_SHA_PRIVATE_IO=1`; drop it for public inputs and outputs. `--rss`
runs each process under `/usr/bin/time` and adds its peak resident set
size, witness generation included, to the summary. `--opener virtual`
selects the virtual opening. `--lambda` defaults to `100` and also accepts
`128` or `sha128-reference-schedule`; `--seed` accepts decimal or `0x`
hex; `--checked` omits the `unchecked` feature. `CARGO_HOME`,
`CARGO_TARGET_DIR` and existing compiler flags are inherited, and
`RUSTFLAGS` defaults to `-C target-cpu=native` when unset.

For a quick smoke run with cached dependencies:

```sh
python3 scripts/run_sha256_bitz_bench.py --offline \
  --shapes 7 --threads 1 --reps 1 --out-dir bench_results/sha256-smoke
```

The bench can also be run directly, for example

```sh
BITZ_BENCH_SHAPES="13 14 15 16" BITZ_BENCH_REPS=5 BITZ_SHA_PRIVATE_IO=1 \
BITZ_LIG_PROFILE=custom:1:4 RAYON_NUM_THREADS=10 \
  cargo bench --bench sha256_compressions --features unchecked,span-metrics
```

## Timing boundaries

- `witness_ms`: circuit replay, source/assignment bit packing and public
  statement materialization. It is not part of the prover time.
- `setup_ms`: one-time public relation preparation and PCS configuration.
- `prove_ms`: the end-to-end prover, from the commitment to the finished
  proof. It includes the commitment, which is also reported alone as
  `commit_ms`; do not add the two.
- `verify_ms`: verification of that proof. Every warm-up and sample is
  verified.
- `witness_to_proof_ms`: the median of each sample's `witness_ms + prove_ms`.

Each `SAMPLE` line also carries `proof_digest`, a digest of both final
transcript states, the commitment root, the opening bytes and the initial
and terminal grinding nonces, so that two builds can be checked to produce
identical proofs.

## Output

Results go into a new directory under `bench_results`, or the new path
given with `--out-dir`; existing paths are rejected. The directory holds
`build.log`, one log per run, `summary.csv` (every `RESULT` field, plus the
run's thread count, profile, statement and opening, `witness_to_proof_ms`
and, with `--rss`, `peak_rss_bytes`), `samples.csv`, and `metadata.json`
with CPU, OS, git revision/status, Rust versions, features, seed, compiler
flags and the relevant environment. The runner clears the SHA-specific
shape/layout/trace overrides and sets the rest itself.

A failed process, a missing result or a wrong number of verified samples
fails the campaign and keeps its logs and the CSV rows written so far.
Runs are sequential, so avoid other CPU heavy work while measuring; the
README campaigns wrap the runner in `scripts/bench_gate.py`.

## Library API

The API is exported from `bitz::piop::spartan`; the benchmark contains a
complete example of this sequence:

```text
prepare_sha256_compression_batch_block_with_profile::<P>(k) -> prepared
  (.with_private_io() for nothing public)
sha256_compression_configs(&prepared) -> prover/verifier configs
generate_sha256_compression_witnesses(&prepared, &inputs) -> witness
Sha256CompressionStatement::new(input, output) -> one statement per input
  (none with private I/O)
commit_sha256_compression_witness_with_config(...) -> commitment + prover hint
prove_sha256_compressions_with_config(...) -> proof
verify_sha256_compressions_with_config(...) -> verification result
```

The batch decides the opening: commit, prove and verify take the block path
for a batch prepared with `prepare_sha256_compression_batch_block_*`, and
the virtual path for `prepare_sha256_compression_batch*`. Use separate
fresh `Blake3Transcript`s for proving and verification.
