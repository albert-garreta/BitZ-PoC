# Binius64 SHA-chain/P-256 comparison

This isolated worker proves the same standard P-256 statement as F2Z and
Spartan2, through either opener: Binius64's own ring switch + BaseFold
(`--method binius64`, `--log-inv-rate 1|3` selects rate 1/2 or 1/8) or the F2Z
opener over the identical circuit and witness (`--method binius64-ligerito` /
`--opener f2z`; `f2z::binius_ligerito::Prepared`, round-by-round 100-bit gate;
the P-256 gadget's select gates lower to 45k BMUL constraints, covered by the
adapter's BinMul reduction term).
It uses the immutable Git revision in this workspace's manifest and
lockfile, with its own Rust toolchain; the parent `f2z` crate is a path
dependency pinned to the identical fork revision (its git revision and dirty
flag are baked into `--build-info`), and the release profile matches the parent
crate (fat LTO, one codegen unit) so neither opener's kernels are handicapped.
The fork builds on Albert Garreta's
`sha-ecdsa-bench`, rebased onto upstream main, and supplies the fixed SHA-chain
and P-256 gadgets. Albert's secp256k1 recovery example remains a distinct workload.

Pinned fork: [`bc73510e`](https://github.com/wu-s-john/binius64/commit/bc73510ed63bf47eec25d4d10ade84f1c1fe2790),
on [`f2z-benching`](https://github.com/wu-s-john/binius64/tree/f2z-benching).
The SHA+ECDSA branch also points to this consolidation commit. The root's other
Binius adapters use the same revision; neither workspace uses a local Binius patch.
Albert's benchmark commit is preserved as `9e880ff4` after rebasing onto upstream
`c28940ae693c3999fd225cc6142f43d07d1100bb`.

## Run

From the F2Z repository root:

```sh
python3 scripts/run_sha256_ecdsa_compare.py \
  --methods f2z-split binius64 binius64-ligerito \
  --spartan-splits 3:0 --targets 100 --threads 1 10 --reps 3 \
  --output bench_results/sha256-ecdsa-binius
```

Binius-family cases run at both rates by default (`--binius-rates 1 3`), the
F2Z rows at both Ligerito profiles (`--f2z-profiles custom:1:4 custom:3:4`).

The worker is optional. Default methods do not build or download its dependencies.
Use `--offline` after dependencies and the pinned toolchain have been installed.
For a prebuilt worker, pass `--binius64-binary PATH` alongside the `.build.json`
sidecar produced by `python3 benchmarks/binius64/build.py`. The runner checks the
executable hash and embedded build metadata. Resume requires matching binaries,
source/lockfile hashes, and standard-P256 fixtures.

The Rust benchmark can also dispatch directly:

```sh
cargo bench --features sha256-ecdsa-compare --bench sha256_ecdsa_compare -- \
  --method binius64 --r 3 --c 0 --target 100 --threads 1 --reps 3 \
  --binius64-worker benchmarks/binius64/target/release/binius64-sha256-ecdsa
```

## Statement and timings

Public: exponent `i` and canonical 32-byte BE integers `Qx,Qy,r,s` (129 logical
bytes, encoded as 17 Binius u64 public words). Private: exactly `64*(2^i-1)`
message bytes. Standard SHA-256 padding adds one block, for `2^i` compressions.
The computed digest is wired into P-256 verification; no digest is public.
Both valid s forms are accepted. Supported exponents are 3 through 16.

Proofs are explicitly non-ZK. The public-input/witness distinction specifies the
relation; it does not promise witness privacy. The worker uses SHA-256 Merkle
hashing and BaseFold at a selectable initial rate, defaulting to 1/2, with an explicit 100/128 FRI query target. Use `--binius-log-inv-rate 1|2|3` on the Python runner (or `--log-inv-rate` on the isolated worker) for rates 1/2, 1/4, and 1/8. The runner records this choice and rejects resuming a directory with a different rate.
Rows record the actual query count and security model separately.

Setup is reusable across messages, keys and signatures. Fixtures/signing are
outside timers. Per-proof timing starts with fresh witness generation and ends
at completed proof bytes. Packing is counted as witness work; ring switching
and deferred BaseFold completion are included in IOP/PCS. Verification checks
the expected public statement and rejects unconsumed proof bytes.

See the [comparison guide](../../docs/sha256-ecdsa-comparison.md) for stage
definitions, process peak memory, raw JSONL, CSV summaries, and provenance.

## Validation

```sh
cargo test --release --features sha256-ecdsa-compare --test sha256_ecdsa_comparison
(cd benchmarks/binius64 && cargo test --release --locked)
python3 -m unittest discover -s scripts -p test_sha256_ecdsa_compare.py
```

The shared tests cover both s forms and a constructed valid signature with
`R.x = n+3`, plus public-input and proof tampering. Fork tests additionally cover
complete point arithmetic, digest boundaries, and constrained division hints.
The worker tests also populate and check the full circuit for a 131,008-byte
message, covering lengths above the legacy 16-bit limit without a large proof run.
