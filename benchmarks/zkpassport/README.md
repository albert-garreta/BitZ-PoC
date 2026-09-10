> **Deprecated:** ZKPassport is retired from active SHA-chain/P-256 comparisons.
> The commands below describe the historical integration; reproduce old results
> from their recorded source revision. New campaigns use standard P-256 fixtures
> and the [F2Z/Spartan2/Binius runner](../../docs/sha256-ecdsa-comparison.md).

# Native non-ZK ZKPassport comparison

The Rust worker in this directory calls `noir_rs` and the native Barretenberg
5.0.0 library directly. There is no Node subprocess, WASM prover, or passport
application logic in a measured sample. The standalone Cargo workspace keeps
the pinned Noir toolchain out of the F2Z crate's normal dependency graph.

Use the sibling `../zk-passport-circuits` checkout on `f2z-benching`. Its
`benchmarks/sha256-ecdsa` package contains the SHA chain and the P-256 circuit.
The circuit is derived from ZKPassport's arithmetic, with explicit canonical
encoding and final coordinate checks. Its README documents these changes
and the remaining upstream compiler diagnostic.

## Run

From the BitZ-pcs root, first build and validate the native runner:

```sh
python3 benchmarks/zkpassport/build.py --test
python3 scripts/run_sha256_ecdsa_compare.py \
  --methods f2z-split zkpassport-honk --exponents 3 5 8 \
  --targets 100 128 --threads 16 --reps 3 \
  --output bench_results/sha256-ecdsa-zkpassport-16t
```

Omit `--exponents` to request the full `i=3..16` sweep (8 through 65,536 SHA
compressions). Default limits are 48 GiB of address space and 3,600 seconds
per worker or Noir compilation, configurable with `--memory-gib` and `--timeout`.
Large circuits may exceed these limits; failed preparations, failed workers,
and timeouts are recorded alongside successful samples. Methods run sequentially
in fresh processes, with one warmup followed by `--reps` measured trials.
Barretenberg runs once per exponent/thread/seed, independently of the F2Z
economic-security target. This campaign uses F2Z Split mode.

Rerun the same command to resume completed work; add `--retry-failed` to retry
failures. Changed binaries, circuit sources, artifacts, or fixtures require
a new output directory. `--offline` requires Rust dependencies, native tools,
Noir dependencies, and the circuit-size-specific SRS already cached.
Existing F2Z/Spartan campaigns retain their defaults when `zkpassport-honk`
is not selected.

The managed bootstrap supports Linux x86_64. It verifies SHA-256 hashes for
the pinned Nargo and Barretenberg release archives and extracted binaries.
If libc++ is absent on a Debian/Ubuntu host, it downloads and extracts the
development/runtime packages into its private cache without installing system
packages. Alternatively pass `--cxx-lib-dir` to `build.py`. Cargo.lock pins
the Rust dependencies; the campaign records the native binary, static-library,
compiler, and lockfile hashes plus build flags.

## Shared relation and timings

The statement is `(i, Qx, Qy, r, s)`, 129 bytes. Each proof demonstrates that
there exists a message of `64*(2^i-1)` bytes whose SHA-256 digest verifies the
P-256 signature. SHA padding is constrained, giving exactly `2^i` compressions.
The digest is computed inside the circuit. Public coordinates and scalars use
canonical big-endian encodings; signatures have `0 < s <= n/2`.

The existing F2Z worker exports a deterministic, versioned JSON fixture. Its
low-s normalization and native signature validation run before all measured
trials. Both workers consume the same fixture and report its content ID.
Application verification includes the same public-domain validation on both
sides, and never uses the private message to verify the proof.

The native path in each trial is:

```rust,ignore
let solved = execute::execute(&artifact.bytecode, encode(&artifact.abi, &fixture)?)?;
let witness = witness::serialize_witness(solved)?; // witness_ms ends here
let proof = api::circuit_prove(&acir, &witness, &vk, &settings)?; // prove_ms
let accepted = api::circuit_verify(&vk, public_inputs, proof.proof, &settings)?;
```

`settings` uses UltraHonk/Poseidon2 with `disable_zk = true`. The PCS is KZG
over BN254. The private input annotation does not make this proof zero knowledge.

- `setup_ms`: ACIR decoding, circuit statistics, SRS initialization, and VK
  preparation, excluding separately reported `srs_download_ms`.
- `witness_ms`: ABI encoding, witness execution, and witness serialization.
- `prove_ms`: native `circuit_prove`, including its per-call circuit/proving-key
  construction. The API does not expose internal stage timings; those fields
  remain null. It is not an isolated cryptographic-kernel timing.
- `witness_to_proof_ms`: the sum for each sample, before computing medians.
- `verify_ms`: public-domain validation and proof verification against expected
  public inputs and the selected circuit's VK.
- `codec_ms`: proof serialization/deserialization, reported separately.
- `proof_material_bytes`: all transmitted proof data. The native wire format
  is an 8-byte length plus 32-byte proof field elements. Public inputs are
  reconstructed from the separately counted 129-byte statement; the reusable
  VK is reported separately and is not charged per proof.
- `peak_rss_bytes`: the whole worker's peak resident memory, including setup
  and all trials. It is not a per-trial allocation measurement.

F2Z's reported economic targets and the BN254 KZG security model use different
accounting. The harness records them separately and does not assign a fabricated
100-bit/128-bit target to Barretenberg.

## Output and validation

Each output directory contains raw JSON/stdout/stderr, all trials in
`samples.csv`, measured-trial medians in `summary.csv`, fixture consistency in
`comparison.json`, and source/build/artifact provenance in `manifest.json`.
Circuit artifacts, fixture files, compiler diagnostics, and source snapshots
are retained. Summaries preserve failures and null measurements.

Run `python3 -m unittest discover -s scripts -p test_sha256_ecdsa_compare.py`
for campaign validation. `build.py --test` runs the shared fixture unit test.
For native negative checks, append `--self-test` to a worker invocation using
the same artifact, fixture, revision, SRS cache, exponent, threads, reps, and
seed from a campaign. This checks changed messages/signatures/keys, high-s,
altered public inputs/VK, and truncated proofs outside measured samples.

The circuit's upstream Brillig constraint-coverage diagnostic is retained.
Passing these tests demonstrates integration and selected boundary cases;
it does not establish a complete soundness audit of the curve library.
