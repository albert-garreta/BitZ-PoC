# Compact Falcon opening validation

The selected correctness checks pass for the initial zero-lane encoding and
the complete final-message encoding. These changes preserve the statement
domains, commitments, challenges, security ledger, and grinding requirements.
Falcon's proof type stores only the compact representation; existing unrelated
generic PCS clients retain their original representation.

This report applies to frozen candidate V7, whose source manifest and build
are recorded in [the V7 source audit](../performance/v7-source-audit.json)
and [build metadata](../performance/candidate-build-v7.json). Concurrent
Falcon parameterization changes in the current working tree are outside
the tested snapshot; these results do not validate that refactor.
[Divergence audit](../performance/v7/working-tree-concurrent-audit.json).
End-to-end timing acceptance is still pending the complete V7 campaign.

| Checks | Result | Evidence |
| --- | --- | --- |
| Falcon modules and exact security ledgers | 135 passed | [log](falcon-v7-tests.log) |
| Joint source, codec, opening, and grinding tests | 16 passed | [log](opening-v7-tests.log) |
| Full supported profile matrix | 28 verified proofs | [log](profile-matrix-v7.log) |
| Generic hybrid clients | 10 passed | [log](generic-hybrid-v7-tests.log) |
| Grinding scanners | 10 passed, 2 ignored | [log](grinding-v7-tests.log) |
| Spartan grinding transcripts | 9 passed | [log](spartan-grinding-v7-tests.log) |
| Vendored Ligerito module | 53 passed, 8 ignored | [log](vendor-ligerito-v2-regression-tests.log) |
| Focused compact terminal tests (included above) | 3 passed | [log](vendor-compact-terminal-v2-tests.log) |
| Vendored Keccak module | 15 passed, 8 ignored | [log](vendor-keccak-contiguous-tests.log) |
| Field crate and doc tests | 119 passed, 1 ignored | [log](public-power/full-field-tests.log) |
| Benchmark affinity controls | 7 passed | [log](affinity-tests-v4.log) |
| Qualification harness | 20 passed | [log](../performance/harness-v4-tests.log) |

The profile matrix covers NativeCarry and SharedPrime at 100 and 128 bits,
with batches 1, 2, 3, 7, 8, 9, and 1024. Root tests use the locked native
release build, sixteen Rayon threads, and one test thread. The profile matrix
was explicitly selected despite its normal ignored status. The generic
hybrid filter also reran eleven Falcon tests; its log therefore reports
21 passing tests and the normally ignored profile matrix.
The latest root executable and exact invocations are recorded in
[root-v7-method.json](root-v7-method.json). Its broader opening filter includes
four grinding and streamed-opening tests in addition to the twelve geometry
and codec tests.

The compact initial-row tests compare reconstructed rows and paths against
the full canonical opening, including batch-one geometry and physical source
padding. They reject full-width rows in the compact encoding, incorrect field
or row counts, lane swaps, modified values, oversized paths, and nonzero data
in a lane that the encoder would omit. The end-to-end joint tests compare
roots, round messages, nonces, and subsequent prover/verifier challenges.

The terminal codec tests cover 64-, 128-, and 256-element messages with both
BLAKE3 and SHA-256. They use an independent dense lane-fold calculation,
reconstruct the ordinary proof and its serialized bytes, and compare subsequent
Fiat–Shamir challenges with OOD and query/fold grinding enabled. Incorrect
message lengths and values, lane permutations, roots, and consumed sumcheck
messages are rejected. Falcon integration also tests compact malformed proofs
under both protocols and targets at batches one and three.

These results cover the second candidate, including public NTT preparation
reuse and exact cancellation of the final induced-query term after complete
root authentication. A separate identity test checks the equality against
dense row folding and the existing induced basis for 64-, 128-, and
256-element messages, code rates 1/2 and 1/64, and zero, one, and non-power-of-two
query counts. It compares the original and simplified residual equations
with arbitrary earlier bases and targets, and checks that an unrelated
residual does not satisfy the authenticated-message identity. The original
explicit-row verifier remains covered by the same module regression tests.
Independent review confirmed that all earlier claims, OOD terms, root checks,
and transcript draws remain in place. First-candidate logs are retained.

The fourth benchmark candidate has identical production sources to the second
candidate. Its only further code changes add optional, benchmark-only CPU
placement and corresponding measurement validation. The baseline receives
the identical benchmark overlay. Affinity tests check actual worker and main
thread masks, startup acknowledgments, invalid requests, and isolation from
the caller's original affinity. They also cover the auxiliary pool and actual
private-verifier thread readback. The placement policy is specific to these
Linux x86_64 Falcon paths: the auxiliary pool's only reached job is serial
Keccak coefficient folding, while parallel work uses the sixteen global
workers. Both pool sizes and the auxiliary dispatch remain unchanged. The
private verifier's actual mask is checked after every trial, outside timers.
The tests' exact reproduction commands and input
hashes are in [affinity-tests-v4-method.json](affinity-tests-v4-method.json).

The fifth candidate additionally keeps Keccak transpose calculations in
contiguous lane order, removes redundant copying for contiguous occupied
source lanes, skips high identity factors in public exponentiation, and
reuses immutable generator preparation. The Keccak tests retain the old
walker as a test-only reference, check all 1,600 basis positions, and compare
with independently constructed coefficients. A gapped two-source geometry
still exercises the general compact-row path. Public-power tests cover zero
and nonzero bases, exponent-width boundaries, composite rings, prime fields,
other window sizes, and private exponentiation against independent binary
powering. The private timing contract is unchanged. The bridge cache is
checked against fresh preparation, and the full Falcon matrix passes with
all four optimizations enabled. Independent review found no issues in these
changes. [Public-power evidence and isolated timings](public-power/README.md),
[Keccak isolated timings](vendor-keccak-contiguous-abba.log).

The sixth candidate also precomputes the public compaction grinding schedule
at compile time using the unchanged exact optimizer. All eleven supported
capacity logs match the previous runtime solver and the frozen expected
schedule tuples. The existing exact soundness-budget checks pass. This removes
repeated schedule computation without changing any grinding difficulty,
search algorithm, nonce, or transcript draw. Independent review passed.
[Schedule evidence](grinding-dispatch/schedule-README.md). Separate scanner
experiments found no benefit from changing the parallel threshold or replacing
broadcast with work-stealing; neither experiment changed production code.

The seventh candidate makes five fixed BLAKE3 message-schedule rounds explicit
constant specializations. This preserves the G operations and their order,
but lets the compiler remove dynamic schedule indexing and bounds checks.
The generic fallback, feature detection, minimum-nonce search, nonce-range
boundaries, and full-hash checks for difficulties above 32 bits are unchanged.
The isolated old/new scanner comparison passed all 1,152 scalar-reference
minimum-nonce cases and the integration tests above. An additional forced-lane
test exercises full-hash rejection near the end of the nonce range.
Independent review passed. [Exact sources, code generation, and isolated
measurements](blake3-fixed-rounds/README.md).

The eight ignored vendor cases are manual size/performance probes and parameter
table generation. They are not counted as passing. This validation does not
claim that every unrelated workspace target was tested.

Vendor tests ran in the existing isolated vendor workspace with its offline
resolved dependencies, using flock-core's default features (no feature flags).
The test-only random challenger is supplied by `cfg(test)`; the transcript
equivalence tests exercise the Fiat–Shamir challenger. The tested and production
`ligerito.rs` files both hash to
`31c4fa0d7fe6da95c1819d4ca0ca3d0e54c80824153d52f50f2000d4d9c1ac84`.
The toolchain was Rust/Cargo 1.98.1, LLVM 22.1.8, x86_64 Linux.

```sh
RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/falcon-vendor-tests RAYON_NUM_THREADS=4 cargo test --manifest-path /tmp/falcon-vendor-workspace/flock-mod/Cargo.toml -p flock-core --release --offline --lib compact_terminal -- --nocapture
RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/falcon-vendor-tests RAYON_NUM_THREADS=4 cargo test --manifest-path /tmp/falcon-vendor-workspace/flock-mod/Cargo.toml -p flock-core --release --offline --lib pcs::ligerito::tests -- --nocapture
```

Runtime acceptance and measured payload savings are recorded separately in
the [performance campaign](../performance/README.md).
