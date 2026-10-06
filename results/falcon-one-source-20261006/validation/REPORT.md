# One-source correctness validation

The final V3 candidate passed the selected correctness checks on Linux x86_64
with native CPU instructions. The root crate used its locked dependencies and
release build. See the [V3 report](v3/README.md) for final logs, independent
reference checks, and the limitations of unrelated existing test fixtures.

| Final V3 checks | Passed | Evidence |
| --- | ---: | --- |
| Falcon modules, including exact security ledgers | 133 | [log](v3/correctness.log) |
| Complete profile matrix | 28 verified proofs | [log](v3/profile-matrix.log) |
| Joint and existing shared openings | 9 | [log](v3/all-opening-tests.log) |
| Existing generic hybrid clients | 10 | [log](v3/generic-hybrid-tests.log) |
| Upstream Falcon interoperability and input cache | 4 | [log](v3/upstream-tests.log) |
| Vendored Keccak constraints, activity, and stripe packing | 15 | [log](v3/vendor-keccak-tests.log) |

The final profile matrix covers both protocols and security targets across
batches 1, 2, 3, 7, 8, 9, and 1024. Whole-proof equality checks also establish
that the V3 optimizations preserve V2 roots, transcripts, payloads, and grinding
diagnostics across 24 configurations. Performance and proof-size acceptance
are measured separately in the [final qualification report](../performance/FINAL.md).

The `falcon_cost_study` and `falcon_size_breakdown` examples type-check. The
pre-existing untracked `proof_size_comparison` prototype still fails because
three APIs it references are absent from both baseline and candidate; it is
not counted as passing. No production feature or repository lockfile was
changed to accommodate the vendor test harness.

## Earlier validation, retained for provenance

The following checks were run before the V2/V3 performance optimizations.
Final checks above supersede the corresponding root and Keccak counts below;
the lincheck and Merkle implementations were unchanged after these checks.

| Checks | Passed | Evidence |
| --- | ---: | --- |
| Falcon modules, including exact security ledgers | 131 | [log](correctness.log) |
| Joint and existing shared opening | 9 | [log](all-opening-tests.log) |
| Upstream Falcon interoperability and input cache | 4 | [log](upstream-tests.log) |
| Complete profile matrix (28 verified proofs) | 1 | [log](profile-matrix.log) |
| Existing generic hybrid clients | 10 | [log](generic-hybrid-tests.log) |
| Vendored Keccak constraints and prefix APIs | 12 | [log](vendor-keccak.log) |
| Vendored lincheck | 11 | [log](vendor-lincheck.log) |
| Vendored Merkle trees | 26 | [log](vendor-merkle.log) |

The complete profile test covers NativeCarry and SharedPrime, both 100- and
128-bit targets, and batches 1, 2, 3, 7, 8, 9 and 1024. Root tests run with
sixteen Rayon threads and one test thread. The normally ignored matrix test
was explicitly selected with `--ignored`; the two unrelated ignored kernel
timing tests were not selected.

The opening tests compare populated-lane encoding and joint Merkle roots
against a dense reference, evaluate projections at non-Boolean points, reject
mutated roots/rows/paths and physical padding, and retain exact proof/transcript
equivalence for existing separate-source clients. Keccak tests exercise the
actual compact constraint walker, constant-column activity masks, every
witness/auxiliary buffer, independently checked outputs, and invalid inactive
computations. Arithmetic tests retain decoder-alias and cross-field checks.

The vendor workspace's existing lockfile is stale, and its test harness needs
the `unsound-challenger` feature to compile existing ignored tests. Vendor
unit tests therefore ran in an isolated source copy with offline dependency
resolution and that test-only feature. Tested modified source hashes match
the repository; the repository lockfile and production features were unchanged.
The selected ordinary prefix tests use `FsChallenger` and independently check
their returned witness claims.

Five pre-existing ignored vendor PCS integration tests were also attempted.
They stop at their stale fixture configuration (`initial_k=4` versus
`log_batch_size=6`), before running the protocol. A new focused prefix test
passes for both ordinary row-major and batch-major clients without depending
on those fixtures. This is a validation limitation, not a passing result for
the ignored tests.

Python checks: all 76 existing Falcon campaign tests and 12 new qualification
harness tests passed. The latter cover the fixed sample policy, paired
arithmetic-mean ratios, incomplete/nonqualifying campaigns, and mean proof-size
gates. `git diff --check` passed. Runtime nonregression is assessed separately
by the fixed-seed performance qualification.
