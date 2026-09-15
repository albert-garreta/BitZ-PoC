# Consolidated Binius64 dependency

The root SHA, integer PCS, multiplication and hybrid adapters share one immutable
Binius64 revision with the isolated SHA-chain/P-256 worker. Both manifests and
lockfiles must move together when this dependency is updated. The root and
worker use Rust 1.98.1; the worker keeps its own workspace, build, and provenance.

The consolidated revision is [`bc73510e`](https://github.com/wu-s-john/binius64/commit/bc73510ed63bf47eec25d4d10ade84f1c1fe2790).
Both published branches point to this commit.

The fork's `f2z-benching` branch merges the old benchmark branch (`2b27daea`)
with `sha256-chain-ecdsa-sig-verify-benching` (`938eadcd`). The latter contains
Albert's benchmark commit, rebased onto upstream `c28940ae`. The merge preserves
both histories and ports the structured Perfetto tags to the current protocol.
It also exposes the two hybrid prefix hooks previously supplied by local patches.
No `vendor/binius64` override remains.

## Protocol migration

Public inputs now contain only instance input/output words; circuit constants
come from setup. The hybrid verifier independently constructs the eight final
SHA state words, observes them in its transcript, checks the public-segment
sumcheck, and checks the claimed wiring evaluation natively. Only the private
trace evaluation remains for the joint sumcheck and shared F2Z opening.

The ordinary Binius prover still completes private ring switching and deferred
BaseFold opening. Trace metadata keeps the `prepare_witness`, `commit_witness`
and `ring_switching` identifiers and adds `public_input_check` and `finish_pcs`.
The SHA+ECDSA worker still uses the same standard P-256 fixture and non-ZK mode.

This consolidation introduced hybrid serialization and statement domain version 4.
The subsequent Johnson/early-OOD merge uses version 5 to combine the new Binius
public-input checks with the new Ligerito geometry and padding authentication.
Regenerate setup and
proof artifacts; earlier proof versions are rejected. SHA+ECDSA fixture schema
is unchanged. Keep historical results as recorded and use a fresh campaign
directory for the consolidated build: old timings describe the old dependency.

## Updating the pin

Update all Binius Git revisions in `Cargo.toml` and
`benchmarks/binius64/Cargo.toml`, regenerate both lockfiles, and rebuild both
binaries. Keep each worker's build metadata with its binary. Validate the hybrid
private-bit evaluation/wiring regression, full hybrid tampering tests, native
Binius SHA and multiplication tests, integer PCS openings, and worker P-256 tests
before collecting new comparison data.

## Validation of this consolidation

The fork passed 18 prover/verifier integration tests, its P-256 circuit tests,
and compilation of Albert's SHA256-sign example/benchmark with Perfetto enabled.
The isolated worker passed all four tests, including both signature forms,
exceptional reductions, tampered public inputs/proofs, and a 131,008-byte message.
Its rebuilt executable completed a warmup and a measured proof at `i=3`.

Root checks cover the hybrid round trip and tampering suite, native u32 range
and product checks, native SHA public-input/proof binding and phase capture,
and integer PCS openings at two shapes/rates. The SHA+ECDSA and native-u32 Python
runner tests also pass. Adapter tests and the three-mode hybrid smoke use release
optimization with LTO disabled and 16 codegen units to shorten validation builds;
they are correctness checks, not replacement paper measurements.
