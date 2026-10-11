# Falcon branch integration, October 5–11, 2026

Integration of the committed Falcon work developed this week, using fetched
`origin` refs and local refs frozen in [branch-inventory.json](branch-inventory.json).
The existing target is lowercase `falcon-poc`, initially `4491309f5` locally and
on origin. Uncommitted work in the original `generic-falcon` checkout and older
September backup branches are outside this merge.

## History deduplication

| Branch or aliases | Pinned tip | Treatment |
|---|---|---|
| `codex/falcon-binder-experiments` | `c3bf4e4e3` | Integration base; contains 105 commits beyond the target, including earlier master ancestry. |
| `codex/falcon-cleanup` | `51b472c55` | Already an ancestor of binder. |
| `codex/falcon-shared-prime`, origin counterpart | `7e24c6f64` | Already an ancestor of binder. |
| `codex/falcon-tail-preparation` | `ac2b42fea` | Already an ancestor of binder. |
| `codex/falcon-witness-preparation`, origin counterpart, `origin/generic-falcon` | `9b7b0d185` | One tip with five unique commits; merged once. |
| `codex/falcon-keccak3-packing`, origin counterpart | `12f8de9e8` | One unique commit; merged once. |
| Local `generic-falcon` | `d865c9df5` | One unique commit; merged with the binder and witness changes. |
| `origin/falcon-grinding-opt` | `4784ee769` | Semantically incorporated by adapted cherry-pick `d9b13bdb4`; recorded ancestry without replaying obsolete implementations. |

The grinding provenance review checked the complete remote patch. Its allocator
and PCS grinding-plan files are identical to the incorporated versions; the
later combined arithmetic relation, protocol versions, shared bridge, and
qualified packing deliberately supersede its old call sites. The ancestry
merge therefore keeps the entire newer integration tree.

## Conflict resolutions

Witness preparation retains generic fixed-size public types and uses reusable
NTT workspaces, typed signatures, direct SHAKE words, and shared validation.
Products are consumed modulo 12289; the exact integer product remains only as a
test oracle. The Keccak branch integrates its Falcon-1024 12+6+2 slab layout and
triple-permutation circuits.

Arithmetic combines compact committed sources and reduced logical domains with
the word binder. Weighted bridge claims are transposed to compact sources without
assuming equality row weights. Experimental compact computation also carries
its own physical matrix row count through the shared weighted-claim API. Its
slower multi-bridge computation remains behind tests/benchmark features.

Bit computation retains the incoming v7/v8 protocol selection. The combined
word-binder/compact-commitment relation uses a distinct `word-binder/v9` domain,
binds its terminal grinding difficulty, and includes its terminal merge in the
composed error budget. Early drops, thread ownership, packed kernels, and both
branches' qualified dispatch selections are retained.

Historical results describe their original revisions. They do not establish
performance of this combined revision, whose proof layout and protocol IDs can
change. Cross-version proof equality is therefore not an integration invariant;
input equality, successful verification, and deterministic proofs across workers
remain required.

## Validation

Completed checks on the integration source:

| Check | Result |
|---|---|
| Falcon library tests, optimized native test profile | 297 passed, 1 ignored |
| Falcon profile integration tests | 13 passed, 8 ignored |
| Shared bridge, grinding, logical/physical padding, marginal task tests | 23 passed |
| Standalone vendored field `q12289` tests, including runtime AVX-512 cases | 14 passed, 1 ignored |
| Serial `--no-default-features --features falcon` library build | Passed |
| Production native arithmetic and full-Falcon release benchmarks | Built successfully |
| Native integration timing screen and additional degree/target checks | 148 verified proofs across 44 processes |
| Arithmetic scaling confirmation | 216 verified proofs across 24 processes |

The native checks cover Falcon-512 batch 1024 at 128 bits and 1/4/8/16 workers;
Falcon-1024 batch 32 at 128 bits and those same workers; and both degrees at
batch 3/100 bits with four workers. Proof contents match across workers within
each executable/input. Inputs match between baseline and candidate. Protocol
changes make cross-version proof equality inappropriate. The baseline binaries
are the frozen `c3bf4e4e3` results from the preceding thread-scaling work.

The first unoptimized library build exposed two stale test imports and a moved
field context in an adapted oracle; these were fixed. The initial unoptimized
run also found an old partial-batch test using batch zero after the new minimum
capacity became one. It now exercises a genuine partial batch of three. That
slow run was interrupted; the subsequent complete optimized native run passed.
No failures are treated as passing tests. Historical patch/log whitespace is
retained byte-for-byte; changed production source passes `git diff --check`.

Main commands (all Cargo invocations used `--offline --locked`):

```sh
RAYON_NUM_THREADS=8 RUSTFLAGS='-C target-cpu=native' \
CARGO_PROFILE_TEST_OPT_LEVEL=2 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=8 \
cargo test --offline --locked --features falcon-hybrid --lib falcon -- --test-threads=1
# Repeat that environment for the profile test only:
cargo test --offline --locked --features falcon-hybrid --test falcon_profiles -- --test-threads=1
# Field and serial checks use their ordinary development profiles:
cargo test --offline --locked --manifest-path vendor/field/Cargo.toml q12289 -- --test-threads=1
cargo check --offline --locked --no-default-features --features falcon --lib
RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=8 \
cargo bench --offline --locked --profile release --features falcon-hybrid \
  --bench falcon_algebraic --bench falcon_hybrid --no-run --message-format=json
python3 native-smoke.py OUTPUT binaries.json
python3 native-smoke.py SCALING_OUTPUT binaries.json scaling
```

The field and serial checks used their ordinary non-native development profiles.
The shared checks directly invoked the compiled development test executable with
four Rayon workers and one test thread. They selected `hybrid::integer_bridge::`,
`hybrid::grinding_allocation::`,
`logical_logs_below_allocation_match_the_full_physical_oracle`,
`small_logical_support_authenticates_all_omitted_physical_words`, and
`marginal_task_shapes_match_direct_sums_across_thread_counts`.

[Source/build manifest](source-build.json) records all 556 source hashes and
native build arguments. [Raw validation evidence](validation-evidence.tar.gz)
contains test/build logs, every native process output, command records, and
process timing bounds. The timing script retains the original host paths;
`binaries.json` supplies the frozen executable paths. Native measurements ran
sequentially after all builds/tests finished. No source changes followed the
native build. Origin was fetched again and every inventoried ref was unchanged.

## Performance limits

[Two-seed timing screen](timing-screen.json), median of per-process medians,
three measured proofs after one warmup, Falcon-512 batch 1024 at 128 bits:

| Variant | Workers | Previous binder ms | Integrated ms | Total change |
|---|---:|---:|---:|---:|
| Arithmetic | 1 | 181.825 | 162.157 | -10.82% |
| Arithmetic | 4 | 65.126 | 59.661 | -8.39% |
| Arithmetic | 8 | 45.070 | 42.282 | -6.19% |
| Arithmetic | 16 | 46.210 | 44.103 | -4.56% |
| Full | 1 | 1957.484 | 1923.107 | -1.76% |
| Full | 4 | 611.812 | 588.589 | -3.80% |
| Full | 8 | 394.362 | 378.783 | -3.95% |
| Full | 16 | 330.748 | 319.134 | -3.51% |

This is a descriptive integration screen, not a complete no-regression
qualification. Every measured cell improved total and prove-only medians against
the preceding binder binary. Full Falcon at 16 workers also beat eight.

**Arithmetic's requested 16-over-8 scaling is not retained in these combined
measurements.** The short screen also reversed the preceding binder's scaling,
so a separate [six-seed confirmation](arithmetic-scaling.json) used seven
measured proofs after two warmups. The integrated arithmetic medians were
**41.799 ms at eight workers and 44.856 ms at sixteen**. The paired geometric
16/8 change was **+4.699%**, with exact six-seed percentile bootstrap interval
**[+1.210%, +8.231%]**; prove-only was **+3.895% [+0.341%, +7.388%]**.
The old binder's scaling was inconclusive in that run. These are descriptive
pointwise intervals, without multiple-comparison correction.

The integrated arithmetic total medians remained below the old binder at both
worker counts in this confirmation (6.75% and 2.78% respectively). The merge is
functionally validated, but the earlier 16-over-8 arithmetic scaling claim must
not be carried forward to this combined branch. A separate performance change
and qualification are needed to recover it.
