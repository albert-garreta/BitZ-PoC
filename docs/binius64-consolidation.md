# Consolidated Binius64 dependency

The root SHA, integer PCS, multiplication and hybrid adapters share one immutable
Binius64 revision with the isolated SHA-chain/P-256 worker. Both manifests and
lockfiles must move together when this dependency is updated. The root and
worker use Rust 1.98.1; the worker keeps its own workspace and build.

Both Cargo manifests pin the same commit of the `wu-s-john/binius64` fork
(`bitz-benching` branch). The fork carries the accumulated benchmark changes,
including structured Perfetto tags and hybrid prefix hooks, above upstream
`binius-zk/binius64`.

## Protocol migration

Public inputs now contain only instance input/output words; circuit constants
come from setup. The hybrid verifier independently constructs the eight final
SHA state words, observes them in its transcript, checks the public-segment
sumcheck, and checks the claimed wiring evaluation natively. Only the private
trace evaluation remains for the joint sumcheck and shared BitZ opening.

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

Keep the root and isolated worker on the same commit. After pushing a change to
the fork, set the new `rev` in `Cargo.toml` and `benchmarks/binius64/Cargo.toml`,
let Cargo re-resolve the two lockfiles (do not run a blanket `cargo update`),
and rebuild both binaries. Worker build metadata records the Binius revision
read from its lockfile, plus the BitZ revision and working-tree state.

Use `bash scripts/compile_export.sh` for compile-only validation. It builds the
worker directly with Cargo, because `benchmarks/binius64/build.py` executes the
worker to collect build metadata. Runtime hybrid, native SHA/multiplication,
integer PCS and P-256 checks are separate validation steps; compilation alone
does not establish their results.
