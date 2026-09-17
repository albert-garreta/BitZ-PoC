# Release source provenance

The release is reconstructed from the public dependency commits pinned by root
revision `8446d64fc011b4b10b6ae852621fd1be4461afaa`, together with the root-tracked
Flock library integration and the packaging intent of `02701f81`. The previously
recorded Mac snapshot commits were not available for comparison. They remain in
`provenance.toml` as `superseded_snapshot_commit`; this release does not claim
source equivalence with those snapshots.

| Vendor | Published fork source | Original upstream |
| --- | --- | --- |
| Limber | `836c50f23e674098dcfbe42a4873f583d4e0fe3f` | `959575409f38a0894dab2f2a7b7125c8aa424b78` |
| Plonky3 | `62f49209aec15ab060c83afbaf9eeb74d8c0c411` | `59be31386d5ab81b87dbceb0b83bf797f9eefaec` |
| Binius64 | `bc73510ed63bf47eec25d4d10ade84f1c1fe2790` | `c28940ae693c3999fd225cc6142f43d07d1100bb` |
| Flock | `ed4c0cd9aceeae15248bf4f24a70d96fe37b2395` | `e636760f8dae78306f804554fb4244993758b011` |

Each `source_fork_tree` records the published source before release adaptations.
Each final `snapshot_tree` includes those adaptations, described in the manifest
and represented in a binary-capable patch. Official upstream URLs are recorded
for reconstruction; builds have no personal-fork dependency.

## History normalization

Only the agreed, verified author/committer identities are replaced by
`bitzcodes <bitzcodes@fastmail.com>`. Four Limber upstream commits have a targeted
author identity; rewriting these and their descendants invalidates 15 commit
signatures, which are removed. Every upstream source tree, parent relationship,
commit message, timestamp, and other contributor identity is preserved. The
other three upstream histories need no identity changes. The original and
normalized tips are recorded separately.

Limber's upstream `main` executable remains in its historical trees. Its removal
is now in the single customization commit, together with removal of tracked
Python bytecode. This replaces the previous approach of removing the executable
throughout history. Every history bundle advertises only `upstream`
and `snapshot`; `snapshot` has exactly one parent, `upstream`. Bundles contain
full history and are distributed separately from the source ZIP.

## Current-source adaptations

- Limber retains its published benchmark instrumentation, aligns the three
  MultiSwap digest domains with the root launcher, reports its verified snapshot
  revision without Git metadata, and keeps the exact official `halo2curves` pin.
- Plonky3 retains its published benchmarks. The optional external `zkhash`
  development dependency and its reference-comparison test are omitted; the
  remaining BN254 tests and library implementation are retained.
- Binius64 retains its published implementation and instrumentation. An incidental
  personal attribution in the SHA/P-256 example introduction is removed.
- Flock retains the root's shared-field and interleaved-oracle implementation.
  The snapshot contains the core and prover libraries, licenses and upstream
  README. Standalone upstream benchmarks, CUDA sources and development targets
  remain omitted, and the workspace manifest lists only retained crates.
- Lockfiles are present for every retained workspace. Existing lockfiles keep
  their dependency versions; newly materialized workspaces receive lockfiles.

## Identifying references deliberately retained

- `crates/LICENSE-MIT` and `vendor/field/LICENSE-MIT` retain the World Foundation
  copyright notices.
- `vendor/limber/README.md` retains Albert Garreta in the cited research authors.
- `benches/field.rs` and `src/poly/univariate/binary_b127.rs` retain Reilabs
  attribution for the benchmark design and field-arithmetic implementation.
- Validation code in `scripts/local_provenance.py`, `scripts/package_release.py`
  and `scripts/test_release_tooling.py` contains the identity/fork patterns it
  must reject. These are checks and synthetic negative fixtures, not dependencies.
- Patches retain upstream text in deletion/context lines, including the removed
  example attribution. Upstream historical contents and commit messages retain
  their original attributions. These are necessary for exact reconstruction.

The root Git history is retained internally. It is neither normalized nor
included in the source ZIP.
